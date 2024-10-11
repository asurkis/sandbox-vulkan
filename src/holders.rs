macro_rules! declare_holder {
    ($typ:ident, $device:ident . $drop_fn:ident) => {
        pub struct $typ<'a>(pub ::ash::vk::$typ, &'a crate::bootstrap::VkBox);

        impl<'a> $typ<'a> {

        }

        impl<'a> Drop for $typ<'a> {
            fn drop(&mut self) {
                unsafe {
                    self.1.$device.$drop_fn(self.0, None);
                }
            }
        }
    };
}

declare_holder!(Fence, device.destroy_fence);
declare_holder!(Semaphore, device.destroy_semaphore);
