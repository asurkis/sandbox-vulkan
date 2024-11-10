#version 450

struct Node {
    uint voxel;
    uint _pad1;
    uint _pad2;
    uint _pad3;
    uvec4 children_packed[2];
};

layout(binding = 1, std140) readonly buffer VoxelOctree {
    uint log_extent;
    Node nodes[];
} tree;

layout(location = 0) in vec3 in_cam_pos;
layout(location = 1) in vec3 in_ray_dir;
layout(location = 0) out vec4 out_color;

uint invert_bit_order(uint x) {
    x = ((x & 0x0000FFFF) << 16) | ((x >> 16) & 0x0000FFFF);
    x = ((x & 0x00FF00FF) << 8) | ((x >> 8) & 0x00FF00FF);
    x = ((x & 0x0F0F0F0F) << 4) | ((x >> 4) & 0x0F0F0F0F);
    x = ((x & 0x33333333) << 2) | ((x >> 2) & 0x33333333);
    x = ((x & 0x55555555) << 1) | ((x >> 1) & 0x55555555);
    return x;
}

uint shrink_bits_3(uint x) {
    x = x & 0x49249249; // 0010_0100_1001
    x = (x | (x >> 2)) & 0xC30C30C3; // 0000_1100_0011
    x = (x | (x >> 4)) & 0x0F00F00F; // 0000_0000_1111
    x = (x | (x >> 8)) & 0xFF0000FF;
    x = (x | (x >> 16)) & 0x0000FFFF;
    return x;
}

vec3 palette_color(uint i) {
    i = invert_bit_order(i);
    uvec3 xyz = uvec3(
        shrink_bits_3(i >> 0),
        shrink_bits_3(i >> 1),
        shrink_bits_3(i >> 2));
    return vec3(1) - vec3(xyz) / vec3(0x3FF);
}

vec3 ray_dir;

bool scan_cube(in vec3 offset, in float extent, out float dist, out vec3 hit_pos, out vec3 hit_color) {
    vec3 proj_min_v = (in_cam_pos - offset) / -ray_dir;
    vec3 proj_max_v = (in_cam_pos - offset - extent) / -ray_dir;
    for (int i = 0; i < 3; ++i) {
        if (ray_dir[i] < 0) {
            float t1 = proj_min_v[i];
            float t2 = proj_max_v[i];
            proj_min_v[i] = t2;
            proj_max_v[i] = t1;
        }
    }

    float proj_min = max(max(proj_min_v.x, proj_min_v.y), proj_min_v.z);
    float proj_max = min(min(proj_max_v.x, proj_max_v.y), proj_max_v.z);
    if (proj_min < 0) return false;
    if (proj_min > proj_max) return false;

    dist = proj_min;
    hit_pos = in_cam_pos + dist * ray_dir;
    hit_color = normalize(hit_pos);
    return true;
}

bool descend_octree(out vec3 hit_pos, out vec3 hit_color) {
    const uint STACK_CAP = 8 * 32;
    uint stack_node_id[STACK_CAP];
    uint stack_log_extent[STACK_CAP];
    vec3 stack_offset[STACK_CAP];

    stack_node_id[0] = 0;
    stack_log_extent[0] = tree.log_extent;
    stack_offset[0] = vec3(0);
    uint stack_len = 1;

    bool found_something = false;
    float best_dist = 0;

    // for (uint z = 0; z < 4; ++z) {
    //     for (uint y = 0; y < 4; ++y) {
    //         for (uint x = 0; x < 4; ++x) {
    //             float cur_dist;
    //             vec3 cur_hit_pos, dummy;
    //             if (scan_cube(vec3(x, y, z), 1, cur_dist, cur_hit_pos, dummy)) {
    //                 if (!found_something || cur_dist < best_dist) {
    //                     best_dist = cur_dist;
    //                     found_something = true;
    //                     hit_pos = cur_hit_pos;
    //                     hit_color = palette_color(16 * z + 4 * y + x);
    //                 }
    //             }
    //         }
    //     }
    // }

    // return found_something;

    while (stack_len > 0) {
        --stack_len;
        uint i_node = stack_node_id[stack_len];
        uint node_log_extent = stack_log_extent[stack_len];
        vec3 node_offset = stack_offset[stack_len];

        float node_dist;
        vec3 node_hit_pos, dummy;
        if (!scan_cube(node_offset, 1 << node_log_extent, node_dist, node_hit_pos, dummy)) {
            continue;
        }

        // if (node_log_extent == 0) {
        if (tree.nodes[i_node].children_packed[0].x == ~0u) {
            // leaf
            // if (length(node_offset - 8) < 8) {
            if (tree.nodes[i_node].voxel != ~0u) {
                if (!found_something || node_dist < best_dist) {
                    hit_pos = node_hit_pos;
                    hit_color = palette_color(i_node);
                    best_dist = node_dist;
                    found_something = true;
                }
            }
        } else {
            // branch
            for (int i_child = 0; i_child < 8; ++i_child) {
                stack_node_id[stack_len] = tree.nodes[i_node].children_packed[(i_child >> 2) & 1][i_child & 3];
                stack_log_extent[stack_len] = node_log_extent - 1;
                stack_offset[stack_len] = node_offset;
                for (int i = 0; i < 3; ++i) {
                    if ((i_child & (1 << i)) != 0) {
                        stack_offset[stack_len][i] += 1 << (node_log_extent - 1);
                    }
                }
                ++stack_len;
            }
        }
    }
    return found_something;
}

void main() {
    ray_dir = normalize(in_ray_dir);

    vec3 hit_pos, color;
    if (descend_octree(hit_pos, color)) {
        out_color = vec4(0.5 * color + 0.5, 1);
    } else {
        out_color = vec4(0.5 * ray_dir + 0.5, 1);
    }
}
