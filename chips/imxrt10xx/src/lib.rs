//! Peripheral implementations for the IMXRT1050 and IMXRT1060 MCUs.
//!
//! imxrt1050 chip: <https://www.nxp.com/design/development-boards/i-mx-evaluation-and-development-boards/i-mx-rt1050-evaluation-kit:MIMXRT1050-EVK>

#![crate_name = "imxrt10xx"]
#![crate_type = "rlib"]
#![no_std]

pub mod chip;
pub mod nvic;

// Peripherals
pub mod ccm;
pub mod ccm_analog;
pub mod dcdc;
pub mod dma;
pub mod gpio;
pub mod gpt;
pub mod iomuxc;
pub mod iomuxc_snvs;
pub mod lpi2c;
pub mod lpuart;

use cortexm7::{
    generic_isr, hard_fault_handler, initialize_ram_jump_to_main, svc_handler, systick_handler,
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

// imxrt 1050 has total of 160 interrupts
#[cfg_attr(all(target_arch = "arm", target_os = "none"), link_section = ".irqs")]
// used Ensures that the symbol is kept until the final binary
#[cfg_attr(all(target_arch = "arm", target_os = "none"), used)]
pub static IRQS: [unsafe extern "C" fn(); 160] = [
    generic_isr, // eDMA (0)
    generic_isr, // eDMA (1)
    generic_isr, // eDMA (2)
    generic_isr, // eDMA (3)
    generic_isr, // eDMA (4)
    generic_isr, // eDMA (5)
    generic_isr, // eDMA (6)
    generic_isr, // eDMA (7)
    generic_isr, // eDMA (8)
    generic_isr, // eDMA (9)
    generic_isr, // eDMA (10)
    generic_isr, // eDMA (11)
    generic_isr, // eDMA (12)
    generic_isr, // eDMA (13)
    generic_isr, // eDMA (14)
    generic_isr, // eDMA (15)
    generic_isr, // Error_interrupt (16)
    generic_isr, // CM7 (17)
    generic_isr, // CM7 (18)
    generic_isr, // CM7 (19)
    generic_isr, // LPUART1 (20)
    generic_isr, // LPUART2 (21)
    generic_isr, // LPUART3 (22)
    generic_isr, // LPUART4 (23)
    generic_isr, // LPUART5 (24)
    generic_isr, // LPUART6 (25)
    generic_isr, // LPUART7 (26)
    generic_isr, // LPUART8 (27)
    generic_isr, // LPI2C1 (28)
    generic_isr, // LPI2C2 (29)
    generic_isr, // LPI2C3 (30)
    generic_isr, // LPI2C4 (31)
    generic_isr, // LPSPI1 (32)
    generic_isr, // LPSPI2 (33)
    generic_isr, // LPSPI3 (34)
    generic_isr, // LPSPI4 (35)
    generic_isr, // FLEXCAN1 (36)
    generic_isr, // FLEXCAN2 (37)
    generic_isr, // CM7 (38)
    generic_isr, // KPP (39)
    generic_isr, // TSC_DIG (40)
    generic_isr, // GPR_IRQ (41)
    generic_isr, // LCDIF (42)
    generic_isr, // CSI (43)
    generic_isr, // PXP (44)
    generic_isr, // WDOG2 (45)
    generic_isr, // SNVS_HP_WRAPPER (46)
    generic_isr, // SNVS_HP_WRAPPER (47)
    generic_isr, // SNVS_HP_WRAPPER / SNVS_LP_WRAPPER (48)
    generic_isr, // CSU (49)
    generic_isr, // DCP (50)
    generic_isr, // DCP (51)
    generic_isr, // DCP (52)
    generic_isr, // TRNG (53)
    generic_isr, // Reserved (54)
    generic_isr, // BEE (55)
    generic_isr, // SAI1 (56)
    generic_isr, // SAI2 (57)
    generic_isr, // SAI3 (58)
    generic_isr, // SAI3 (59)
    generic_isr, // SPDIF (60)
    generic_isr, // PMU (61)
    generic_isr, // Reserved (62)
    generic_isr, // Temperature Monitor (63)
    generic_isr, // Temperature Monitor (64)
    generic_isr, // USB PHY (65)
    generic_isr, // USB PHY (66)
    generic_isr, // ADC1 (67)
    generic_isr, // ADC2 (68)
    generic_isr, // DCDC (69)
    generic_isr, // Reserved (70)
    generic_isr, // Reserved (71)
    generic_isr, // GPIO1 (72)
    generic_isr, // GPIO1 (73)
    generic_isr, // GPIO1 (74)
    generic_isr, // GPIO1 (75)
    generic_isr, // GPIO1 (76)
    generic_isr, // GPIO1 (77)
    generic_isr, // GPIO1 (78)
    generic_isr, // GPIO1 (79)
    generic_isr, // GPIO1_1 (80)
    generic_isr, // GPIO1_2 (81)
    generic_isr, // GPIO2_1 (82)
    generic_isr, // GPIO2_2 (83)
    generic_isr, // GPIO3_1 (84)
    generic_isr, // GPIO3_2 (85)
    generic_isr, // GPIO4_1 (86)
    generic_isr, // GPIO4_2 (87)
    generic_isr, // GPIO5_1 (88)
    generic_isr, // GPIO5_2 (89)
    generic_isr, // FLEXIO1 (90)
    generic_isr, // FLEXIO2 (91)
    generic_isr, // WDOG1 (92)
    generic_isr, // RTWDOG (93)
    generic_isr, // EWM (94)
    generic_isr, // CCM (95)
    generic_isr, // CCM (96)
    generic_isr, // GPC (97)
    generic_isr, // SRC (98)
    generic_isr, // Reserved (99)
    generic_isr, // GPT1 (100)
    generic_isr, // GPT2 (101)
    generic_isr, // FLEXPWM1 (102)
    generic_isr, // FLEXPWM1 (103)
    generic_isr, // FLEXPWM1 (104)
    generic_isr, // FLEXPWM1 (105)
    generic_isr, // FLEXPWM1 (106)
    generic_isr, // Reserved (107)
    generic_isr, // FLEXSPI (108)
    generic_isr, // SEMC (109)
    generic_isr, // USDHC1 (110)
    generic_isr, // USDHC2 (111)
    generic_isr, // USB (112)
    generic_isr, // USB (113)
    generic_isr, // ENET (114)
    generic_isr, // ENET (115)
    generic_isr, // XBAR1 (116)
    generic_isr, // XBAR1 (117)
    generic_isr, // ADC_ETC (118)
    generic_isr, // ADC_ETC (119)
    generic_isr, // ADC_ETC (120)
    generic_isr, // ADC_ETC (121)
    generic_isr, // PIT (122)
    generic_isr, // ACMP (123)
    generic_isr, // ACMP (124)
    generic_isr, // ACMP (125)
    generic_isr, // ACMP (126)
    generic_isr, // Reserved (127)
    generic_isr, // Reserved (128)
    generic_isr, // ENC1 (129)
    generic_isr, // ENC2 (130)
    generic_isr, // ENC3 (131)
    generic_isr, // ENC4 (132)
    generic_isr, // QTIMER1 (133)
    generic_isr, // QTIMER2 (134)
    generic_isr, // QTIMER3 (135)
    generic_isr, // QTIMER4 (136)
    generic_isr, // FLEXPWM2 (137)
    generic_isr, // FLEXPWM2 (138)
    generic_isr, // FLEXPWM2 (139)
    generic_isr, // FLEXPWM2 (140)
    generic_isr, // FLEXPWM2 (141)
    generic_isr, // FLEXPWM3 (142)
    generic_isr, // FLEXPWM3 (143)
    generic_isr, // FLEXPWM3 (144)
    generic_isr, // FLEXPWM3 (145)
    generic_isr, // FLEXPWM3 (146)
    generic_isr, // FLEXPWM4 (147)
    generic_isr, // FLEXPWM4 (148)
    generic_isr, // FLEXPWM4 (149)
    generic_isr, // FLEXPWM4 (150)
    generic_isr, // FLEXPWM4 (151)
    generic_isr, // Reserved (152)
    generic_isr, // Reserved (153)
    generic_isr, // Reserved (154)
    generic_isr, // Reserved (155)
    generic_isr, // Reserved (156)
    generic_isr, // Reserved (157)
    generic_isr, // Reserved (158)
    generic_isr, // Reserved (159)
];

pub unsafe fn init() {
    cortexm7::nvic::disable_all();
    cortexm7::nvic::clear_all_pending();

    cortexm7::scb::set_vector_table_offset(
        &BASE_VECTORS as *const [unsafe extern "C" fn(); 16] as *const (),
    );

    cortexm7::nvic::enable_all();
}
