//! Tools for displaying process state.

use core::cell::Cell;
use core::fmt::Write;

use crate::process::Process;
use crate::utilities::cells::NumericCellExt;

/// Trait for creating a custom "process printer" that formats process state in
/// some sort of presentable format.
///
/// Typically, implementations will display process state in a text UI over some
/// sort of terminal.
///
/// This trait also allows for experimenting with different process display
/// formats. For example, some use cases might want more or less detail, or to
/// encode the process state in some sort of binary format that can be expanded
/// into a human readable format later. Other cases might want to log process
/// state to nonvolatile storage rather than display it immediately.
pub trait ProcessPrinter {
    fn print(&self, process: &dyn Process, writer: &mut dyn Write) -> bool;
}

pub struct ProcessPrinterText {
    iteration: Cell<usize>,
}

impl ProcessPrinterText {
    pub fn new() -> ProcessPrinterText {
        ProcessPrinterText {
            iteration: Cell::new(0),
        }
    }
}

impl ProcessPrinter for ProcessPrinterText {
    fn print(&self, process: &dyn Process, writer: &mut dyn Write) -> bool {
        let iter = self.iteration.get();
        self.iteration.increment();

        match iter {
            0 => {
                // Process statistics
                let events_queued = 0; //self.tasks.map_or(0, |tasks| tasks.len());
                let syscall_count = process.debug_syscall_count();
                let dropped_upcall_count = process.debug_dropped_upcall_count();
                let restart_count = process.get_restart_count();

                let _ = writer.write_fmt(format_args!(
                    "\
                     𝐀𝐩𝐩: {}   -   [{:?}]\
                     \r\n Events Queued: {}   Syscall Count: {}   Dropped Upcall Count: {}\
                     \r\n Restart Count: {}\r\n",
                    process.get_process_name(),
                    process.get_state(),
                    events_queued,
                    syscall_count,
                    dropped_upcall_count,
                    restart_count,
                ));

                true
            }
            1 => {
                let sram_end = process.mem_end() as usize;

                let _ = writer.write_fmt(format_args!(
                    "\
                     \r\n\
                     \r\n ╔═══════════╤══════════════════════════════════════════╗\
                     \r\n ║  Address  │ Region Name    Used | Allocated (bytes)  ║\
                     \r\n ╚{:#010X}═╪══════════════════════════════════════════╝",
                    sram_end,
                ));

                true
            }
            2 => {
                let grant_ptr_size = 0; //mem::size_of::<GrantPointerEntry>();
                let grant_ptrs_num = 0; //self.kernel.get_grant_count_and_finalize();
                let sram_grant_pointers_size = grant_ptrs_num * grant_ptr_size;

                let sram_upcall_list_size = 0; //Self::CALLBACKS_OFFSET;
                let sram_process_struct_size = 0; //Self::PROCESS_STRUCT_OFFSET;

                let _ = writer.write_fmt(format_args!(
                    "\
                     \r\n             │ Grant Ptrs   {:6}\
                     \r\n             │ Upcalls      {:6}\
                     \r\n             │ Process      {:6}",
                    sram_grant_pointers_size, sram_upcall_list_size, sram_process_struct_size,
                ));

                true
            }

            3 => {
                // SRAM addresses
                let sram_end = process.mem_end() as usize;
                let sram_grant_pointers_start = sram_end - 52; //sram_grant_pointers_size;
                let sram_upcall_list_start = sram_grant_pointers_start - 0; //Self::CALLBACKS_OFFSET;
                let process_struct_memory_location = sram_upcall_list_start - 0; // Self::PROCESS_STRUCT_OFFSET;
                let sram_grant_start = process.kernel_memory_break() as usize;
                let sram_heap_end = process.app_memory_break() as usize;

                // SRAM sizes
                let sram_grant_size = process_struct_memory_location - sram_grant_start;
                let sram_grant_allocated = process_struct_memory_location - sram_grant_start;

                let _ = writer.write_fmt(format_args!(
                    "\
                     \r\n  {:#010X} ┼───────────────────────────────────────────\
                     \r\n             │ ▼ Grant      {:6} | {:6}{}\
                     \r\n  {:#010X} ┼───────────────────────────────────────────\
                     \r\n             │ Unused\
                     \r\n  {:#010X} ┼───────────────────────────────────────────",
                    process_struct_memory_location,
                    sram_grant_size,
                    sram_grant_allocated,
                    exceeded_check(sram_grant_size, sram_grant_allocated),
                    sram_grant_start,
                    sram_heap_end,
                ));

                self.iteration.set(0);
                false
            }

            _ => {
                // Should never happen, all valid iteration cases should be
                // handled.
                false
            }
        }
    }
}

fn exceeded_check(size: usize, allocated: usize) -> &'static str {
    if size > allocated {
        " EXCEEDED!"
    } else {
        "          "
    }
}
