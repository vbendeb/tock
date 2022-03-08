//! Peripheral implementations for the STM32F4xx MCU.
//!
//! STM32F446RE: <https://www.st.com/en/microcontrollers/stm32f4.html>

#![crate_name = "stm32f4xx"]
#![crate_type = "rlib"]
#![no_std]

pub mod chip;
pub mod nvic;

// Peripherals
pub mod adc;
pub mod dbg;
pub mod deferred_calls;
pub mod dma1;
pub mod exti;
pub mod fsmc;
pub mod gpio;
pub mod i2c;
pub mod rcc;
pub mod spi;
pub mod syscfg;
pub mod tim2;
pub mod trng;
pub mod usart;

use cortexm4::{
    hard_fault_handler, initialize_ram_jump_to_main, svc_handler, systick_handler,
    unhandled_interrupt,
};

extern "C" {
    // _estack is not really a function, but it makes the types work
    // You should never actually invoke it!!
    fn _estack();
}

#[cfg_attr(
    all(target_arch = "arm", target_os = "none"),
    link_section = ".vectors"
)]
// used Ensures that the symbol is kept until the final binary
#[cfg_attr(all(target_arch = "arm", target_os = "none"), used)]
pub static BASE_VECTORS: [unsafe extern "C" fn(); 16] = [
    _estack,
    initialize_ram_jump_to_main,
    unhandled_interrupt, // NMI
    hard_fault_handler,  // Hard Fault
    unhandled_interrupt, // MemManage
    unhandled_interrupt, // BusFault
    unhandled_interrupt, // UsageFault
    unhandled_interrupt,
    unhandled_interrupt,
    unhandled_interrupt,
    unhandled_interrupt,
    svc_handler,         // SVC
    unhandled_interrupt, // DebugMon
    unhandled_interrupt,
    unhandled_interrupt, // PendSV
    systick_handler,     // SysTick
];

pub unsafe fn init() {
    cortexm4::nvic::disable_all();
    cortexm4::nvic::clear_all_pending();
    cortexm4::nvic::enable_all();
}
