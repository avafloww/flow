use core::fmt;
use crate::driver::DriverLoadOrder;
use crate::{info, println, todo_print};
use crate::exception::asynchronous::IRQNumber;
use crate::sync::interface::Mutex;
use crate::sync::{IRQSafeNullLock};

static DRIVER_MANAGER: DriverManager<IRQNumber> = DriverManager::new();

pub fn driver_manager() -> &'static DriverManager<IRQNumber> {
    &DRIVER_MANAGER
}

const MAX_DRIVERS: usize = 32;

pub type DeviceDriverPostInitCallback = unsafe fn() -> Result<(), &'static str>;

#[derive(Copy, Clone)]
pub struct DeviceDriverDescriptor<T> where T: 'static {
    device_driver: &'static (dyn super::interface::DeviceDriver<IRQNumberType=T> + Sync),
    post_init_callback: Option<DeviceDriverPostInitCallback>,
    irq_number: Option<&'static T>,
    init_complete: bool,
}

impl<T> DeviceDriverDescriptor<T> {
    pub const fn new(
        device_driver: &'static (dyn super::interface::DeviceDriver<IRQNumberType=T> + Sync),
        post_init_callback: Option<DeviceDriverPostInitCallback>,
        irq_number: Option<&'static T>,
    ) -> Self {
        Self {
            device_driver,
            post_init_callback,
            irq_number,
            init_complete: false,
        }
    }
}

struct DriverManagerInner<T> where T: 'static {
    next_index: usize,
    descriptors: [Option<DeviceDriverDescriptor<T>>; MAX_DRIVERS],
}

pub struct DriverManager<T> where T: 'static {
    inner: IRQSafeNullLock<DriverManagerInner<T>>,
}

impl<T> DriverManagerInner<T> where T: 'static + Copy {
    pub const fn new() -> Self {
        Self {
            next_index: 0,
            descriptors: [None; MAX_DRIVERS],
        }
    }
}

impl<T> DriverManager<T> where T: fmt::Display + Copy {
    pub const fn new() -> Self {
        Self {
            inner: IRQSafeNullLock::new(DriverManagerInner::new()),
        }
    }

    pub fn register(&self, descriptor: DeviceDriverDescriptor<T>) {
        self.inner.lock(|inner| {
            inner.descriptors[inner.next_index] = Some(descriptor);
            inner.next_index += 1;
        })
    }

    pub fn enumerate(&self) {
        let mut i: usize = 1;
        self.for_each(|descriptor| {
            info!("    {}. {}", i, descriptor.device_driver.compatible());
            i += 1;
        });
    }

    pub fn init_interrupt_controller(&self) {
        self.probe_devices(DriverLoadOrder::InterruptController);
        unsafe { self.init_devices(DriverLoadOrder::InterruptController) }
    }

    pub fn init_early(&self) {
        self.probe_devices(DriverLoadOrder::Early);
        unsafe { self.init_devices(DriverLoadOrder::Early); }
    }

    pub fn init_normal(&self) {
        self.probe_devices(DriverLoadOrder::Normal);
        unsafe { self.init_devices(DriverLoadOrder::Normal); }
    }

    unsafe fn init_devices(&self, load_order: DriverLoadOrder) {
        self.for_each_mut(|descriptor| {
            if descriptor.init_complete || descriptor.device_driver.load_order() != load_order {
                return;
            }

            if let Err(x) = descriptor.device_driver.init(descriptor.irq_number) {
                panic!("Failed to init driver: {}: {}", descriptor.device_driver.compatible(), x);
            }

            if let Some(callback) = descriptor.post_init_callback {
                if let Err(x) = callback() {
                    panic!("Error during driver post-init callback: {}: {}", descriptor.device_driver.compatible(), x);
                }
            }

            descriptor.init_complete = true;
        });
    }

    fn probe_devices(&self, load_order: DriverLoadOrder) {
        println!("initialising device probe (load order: {:?})", load_order);

        todo_print!("probe devices");
        // on ARM, we probe the device tree for info on devices
        #[cfg(not(target_arch = "aarch64"))]
        compile_error!("Add the target_arch to above's check if the following code is safe to use");
        // let dtb = unsafe { Dtb::from_raw_parts(*DTB_PTR_ADDR as *const u8) }
        //     .unwrap_or_else(|e| panic!("Failed to parse device tree: {:?}", e));

        // dtb.walk(|path, obj| match obj {
        //     DtbObj::SubNode { name } => {
        //         let name_str = core::str::from_utf8(name).unwrap_or("");
        //         println!("sub - {path}/{name_str}");
        //         WalkOperation::StepInto
        //     }
        //     DtbObj::Property(prop) => {
        //         println!("prop - {path}/{prop:?}");
        //         WalkOperation::StepInto
        //     }
        // });
    }

    fn for_each<'a>(&'a self, f: impl FnMut(&'a DeviceDriverDescriptor<T>)) {
        self.inner.lock(|inner| {
            inner.descriptors.iter().filter_map(|x| x.as_ref()).for_each(f)
        })
    }

    pub fn for_each_mut<'a>(&'a self, f: impl FnMut(&'a mut DeviceDriverDescriptor<T>)) {
        self.inner.lock(|inner| {
            inner.descriptors.iter_mut().filter_map(|x| x.as_mut()).for_each(f)
        })
    }
}
