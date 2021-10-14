//! Provide a console over a multiplexed UART channel.

use core::cell::Cell;
use core::cmp;

use kernel::collections::list::{List, ListLink, ListNode};
use kernel::dynamic_deferred_call::{
    DeferredCallHandle, DynamicDeferredCall, DynamicDeferredCallClient,
};
use kernel::grant::Grant;
use kernel::hil::uart;
use kernel::processbuffer::{ReadOnlyProcessBuffer, ReadWriteProcessBuffer};
use kernel::processbuffer::{ReadableProcessBuffer, WriteableProcessBuffer};
use kernel::syscall::{CommandReturn, SyscallDriver};
use kernel::utilities::cells::{OptionalCell, TakeCell};
use kernel::{ErrorCode, ProcessId};

const RX_BUF_LEN: usize = 64;
pub static mut RX_BUF: [u8; RX_BUF_LEN] = [0; RX_BUF_LEN];

/// Syscall driver number.
use crate::driver;
pub const DRIVER_NUM: usize = driver::NUM::Console as usize;

/// Denotes what is using the underlying UART currently. Can either be an
/// in-kernel user or a specific process.
enum ActiveUser<'a> {
    KernelUser(&'a UartDevice<'a>),
    ProcessUser(ProcessId),
}

#[derive(Copy, Clone, PartialEq)]
enum TransmitOperation {
    Transmit { len: usize },
    TransmitWord { word: u32 },
}

#[derive(Copy, Clone, PartialEq)]
enum UartDeviceReceiveState {
    Idle,
    Receiving,
    Aborting,
}

/// The grant state stored for each process using the SuperConsole.
#[derive(Default)]
pub struct App {
    /// Shared buffer for transmitted bytes.
    write_buffer: ReadOnlyProcessBuffer,
    /// Total bytes to send from the `write_buffer`. Set when the process calls
    /// the send command.
    write_len: usize,
    /// Number of bytes from `write_buffer` that have yet to be sent. If this is
    /// greater than 0 then this process has a pending send.
    write_remaining: usize,

    read_buffer: ReadWriteProcessBuffer,
    read_len: usize,
}

/// SuperConsole manages both a list of in-kernel UART users as well as a list
/// of process UART (console) users.
pub struct SuperConsole<'a> {
    /// Underlying UART implementation for send and receive.
    uart: &'a dyn uart::UartData<'a>,
    /// Use deferred calls to provide error callbacks asynchronously.
    deferred_caller: &'a DynamicDeferredCall,
    /// Our deferred call handle set after this module is registered as a
    /// deferred call.
    deferred_call_handle: OptionalCell<DeferredCallHandle>,
    /// List of UART users in the kernel.
    kernel_users: List<'a, UartDevice<'a>>,
    /// Need a grant to hold per-process state. Use 3 upcalls.
    apps: Grant<App, 3>,
    /// Transmit buffer passed to the UART hardware for transmissions from
    /// processes. Since processes share with us a ProcessBuffer, we need a
    /// static buffer to copy into before passing data to the UART. For
    /// in-kernel users they will provide us with a static buffer which we can
    /// pass directly to the UART.
    tx_buffer: TakeCell<'static, [u8]>,
    /// Receive buffer passed to the UART to hold any incoming receive messages.
    rx_buffer: TakeCell<'static, [u8]>,
    /// What is currently using the underlying UART.
    inflight: OptionalCell<ActiveUser>,

    completing_read: Cell<bool>,
    tx_in_progress: OptionalCell<ProcessId>,
    rx_in_progress: OptionalCell<ProcessId>,
}

// pub struct Console<'a> {
//     uart: &'a dyn uart::UartData<'a>,

//     tx_buffer: TakeCell<'static, [u8]>,

//     rx_buffer: TakeCell<'static, [u8]>,
// }

// impl<'a> Console<'a> {
//     pub fn new(
//         uart: &'a dyn uart::UartData<'a>,
//         tx_buffer: &'static mut [u8],
//         rx_buffer: &'static mut [u8],
//         grant: Grant<App, 3>,
//     ) -> Console<'a> {
//         Console {
//             uart: uart,

//             tx_in_progress: OptionalCell::empty(),
//             tx_buffer: TakeCell::new(tx_buffer),
//             rx_in_progress: OptionalCell::empty(),
//             rx_buffer: TakeCell::new(rx_buffer),
//         }
//     }

impl<'a> SuperConsole<'a> {
    pub fn new(
        uart: &'a dyn uart::UartData<'a>,
        deferred_caller: &'a DynamicDeferredCall,
        grant: Grant<App, 3>,
        tx_buffer: &'static mut [u8],
        rx_buffer: &'static mut [u8],
    ) -> SuperConsole<'a> {
        SuperConsole {
            uart: uart,
            deferred_caller: deferred_caller,
            handle: OptionalCell::empty(),
            kernel_users: List::new(),
            apps: grant,
            tx_buffer: TakeCell::new(tx_buffer),
            rx_buffer: TakeCell::new(rx_buffer),
            inflight: OptionalCell::empty(),

            completing_read: Cell::new(false),
            tx_in_progress: OptionalCell::empty(),
            rx_in_progress: OptionalCell::empty(),
        }
    }

    pub fn initialize_callback_handle(&self, handle: DeferredCallHandle) {
        self.deferred_call_handle.replace(handle);
    }

    /// If nothing is active, then check if there are any pending transmissions,
    /// and if so, transmit the first pending transmission found.
    fn do_next_transmission(&self) {
        if self.inflight.is_none() {
            // Check for in-kernel users first.
            let pending_kernel_user = self
                .kernel_users
                .iter()
                .find(|node| node.operation.is_some());
            pending_kernel_user.map_or_else(
                || {
                    // No in-kernel users are pending. Now check for processes
                    // with pending transmissions.
                    for cntr in self.apps.iter() {
                        let appid = cntr.processid();
                        let started_tx = cntr.enter(|app, upcalls| {
                            if app.write_remaining > 0 {
                                self.send(app_id, app);
                                true
                            } else {
                                false
                            }
                        });
                        if started_tx {
                            self.inflight.set(ActiveUser::ProcessUser(appid));
                            break;
                        }
                    }
                },
                |node| {
                    // A pending in-kernel user was found. Handle its
                    // transmission.
                    node.tx_buffer.take().map(|buf| {
                        node.operation.map(move |op| match op {
                            TransmitOperation::Transmit { len } => {
                                let _ = self.uart.transmit_buffer(buf, *len).map_err(
                                    move |(ecode, buf)| {
                                        node.tx_client.map(move |client| {
                                            node.transmitting.set(false);
                                            client.transmitted_buffer(buf, 0, Err(ecode));
                                        });
                                    },
                                );
                            }
                            TransmitOperation::TransmitWord { word } => {
                                let rcode = self.uart.transmit_word(*word);
                                if rcode != Ok(()) {
                                    node.tx_client.map(|client| {
                                        node.transmitting.set(false);
                                        client.transmitted_word(rcode);
                                    });
                                }
                            }
                        });
                    });
                    node.operation.clear();
                    self.inflight.set(ActiveUser::KernelUser(node));
                },
            );
        }
    }

    /// Internal helper function for continuing a previously set up transaction.
    /// Returns true if this send is still active, or false if it has completed.
    fn process_send_continue(&self, app_id: ProcessId, app: &mut App) -> bool {
        if app.write_remaining > 0 {
            self.send(app_id, app);
            true
        } else {
            false
        }
    }

    /// Starts a new UART reception, return value denotes whether starting
    /// the reception will issue a callback before the new read. A callback
    /// needs to be issued before the new read if a read was ongoing; the
    /// callback finishes the current read so the new one can start.
    ///
    /// Three cases:
    /// 1. We are in the midst of completing a read: let the `received_buffer()`
    ///    handler restart the reads if needed (return false)
    /// 2. We are in the midst of a read: abort so we can start a new read now
    ///    (return true)
    /// 3. We are idle: start reading (return false)
    fn start_receive(&self, rx_len: usize) -> bool {
        self.buffer.take().map_or_else(
            || {
                // No rxbuf which means a read is ongoing
                if self.completing_read.get() {
                    // Case (1). Do nothing here, `received_buffer()` handler
                    // will call start_receive when ready.
                    false
                } else {
                    // Case (2). Stop the previous read so we can use the
                    // `received_buffer()` handler to recalculate the minimum
                    // length for a read.
                    let _ = self.uart.receive_abort();
                    true
                }
            },
            |rxbuf| {
                // Case (3). No ongoing receive calls, we can start one now.
                let len = cmp::min(rx_len, rxbuf.len());
                let _ = self.uart.receive_buffer(rxbuf, len);
                false
            },
        )
    }

    /// Asynchronously executes the next operation, if any. Used by calls
    /// to trigger do_next_op such that it will execute after the call
    /// returns. This is important in case the operation triggers an error,
    /// requiring a callback with an error condition; if the operation
    /// is executed synchronously, the callback may be reentrant (executed
    /// during the downcall). Please see
    ///
    /// https://github.com/tock/tock/issues/1496
    fn do_next_op_async(&self) {
        self.handle.map(|handle| self.deferred_caller.set(*handle));
    }
}

impl<'a> uart::TransmitClient for SuperConsole<'a> {
    fn transmitted_buffer(
        &self,
        tx_buffer: &'static mut [u8],
        tx_len: usize,
        rcode: Result<(), ErrorCode>,
    ) {
        self.inflight.map(move |device| {
            self.inflight.clear();
            device.transmitted_buffer(tx_buffer, tx_len, rcode);
        });
        self.do_next_op();
    }
}

impl<'a> uart::ReceiveClient for SuperConsole<'a> {
    fn received_buffer(
        &self,
        buffer: &'static mut [u8],
        rx_len: usize,
        rcode: Result<(), ErrorCode>,
        error: uart::Error,
    ) {
        // Likely we will issue another receive in response to the previous one
        // finishing. `next_read_len` keeps track of the shortest outstanding
        // receive requested by any client. We start with the longest it can be,
        // i.e. the length of the buffer we pass to the UART.
        let mut next_read_len = buffer.len();
        let mut read_pending = false;

        // Set a flag that we are in this callback handler. This allows us to
        // note that we can wait until all callbacks are finished before
        // starting a new UART receive.
        self.completing_read.set(true);

        // Because clients may issue another read in their callback we need to
        // first copy out all the data, then make the callbacks.
        //
        // Multiple client reads of different sizes can be pending. This code
        // copies the underlying UART read into each of the client buffers.
        self.devices.iter().for_each(|device| {
            if device.receiver {
                device.rx_buffer.take().map(|rxbuf| {
                    let state = device.state.get();
                    // Copy the read into the buffer starting at rx_position
                    let position = device.rx_position.get();
                    let remaining = device.rx_len.get() - position;
                    let len = cmp::min(rx_len, remaining);
                    if state == UartDeviceReceiveState::Receiving
                        || state == UartDeviceReceiveState::Aborting
                    {
                        // debug!("Have {} bytes, copying in bytes {}-{}, {} remain", rx_len, position, position + len, remaining);
                        for i in 0..len {
                            rxbuf[position + i] = buffer[i];
                        }
                    }
                    device.rx_position.set(position + len);
                    device.rx_buffer.replace(rxbuf);
                });
            }
        });
        // If the underlying read completes a client read, issue a callback to
        // that client. In the meanwhile, compute the length of the next
        // underlying UART read as the shortest outstanding read, including and
        // new reads setup in the callback. If any client has more to read or
        // has started a new read, issue another underlying UART receive.
        self.devices.iter().for_each(|device| {
            if device.receiver {
                device.rx_buffer.take().map(|rxbuf| {
                    let state = device.state.get();
                    let position = device.rx_position.get();
                    let remaining = device.rx_len.get() - position;
                    // If this finishes the read, signal to the caller,
                    // otherwise update state so next read will fill in
                    // more data.
                    if remaining == 0 {
                        device.state.set(UartDeviceReceiveState::Idle);
                        device.received_buffer(rxbuf, position, rcode, error);
                        // Need to check if receive was called in callback
                        if device.state.get() == UartDeviceReceiveState::Receiving {
                            read_pending = true;
                            next_read_len = cmp::min(next_read_len, device.rx_len.get());
                        }
                    } else if state == UartDeviceReceiveState::Aborting {
                        device.state.set(UartDeviceReceiveState::Idle);
                        device.received_buffer(
                            rxbuf,
                            position,
                            Err(ErrorCode::CANCEL),
                            uart::Error::Aborted,
                        );
                        // Need to check if receive was called in callback
                        if device.state.get() == UartDeviceReceiveState::Receiving {
                            read_pending = true;
                            next_read_len = cmp::min(next_read_len, device.rx_len.get());
                        }
                    } else {
                        device.rx_buffer.replace(rxbuf);
                        next_read_len = cmp::min(next_read_len, remaining);
                        read_pending = true;
                    }
                });
            }
        });

        // After we have finished all callbacks we can replace this buffer. We
        // have to wait to replace this to make sure that a client calling
        // `receive_buffer()` in its callback does not start an underlying UART
        // receive before all callbacks have finished.
        self.buffer.replace(buffer);

        // Clear the flag that we are in this handler.
        self.completing_read.set(false);

        // If either our outstanding receive was longer than the number of bytes
        // we just received, or if a new receive has been started, we start the
        // underlying UART receive again.
        if read_pending {
            self.start_receive(next_read_len);
        }
    }
}

impl<'a> DynamicDeferredCallClient for SuperConsole<'a> {
    fn call(&self, _handle: DeferredCallHandle) {
        self.do_next_op();
    }
}

pub struct UartDevice<'a> {
    state: Cell<UartDeviceReceiveState>,
    mux: &'a SuperConsole<'a>,
    receiver: bool, // Whether or not to pass this UartDevice incoming messages.
    tx_buffer: TakeCell<'static, [u8]>,
    transmitting: Cell<bool>,
    rx_buffer: TakeCell<'static, [u8]>,
    rx_position: Cell<usize>,
    rx_len: Cell<usize>,
    operation: OptionalCell<Operation>,
    next: ListLink<'a, UartDevice<'a>>,
    rx_client: OptionalCell<&'a dyn uart::ReceiveClient>,
    tx_client: OptionalCell<&'a dyn uart::TransmitClient>,
}

impl<'a> UartDevice<'a> {
    pub const fn new(mux: &'a SuperConsole<'a>, receiver: bool) -> UartDevice<'a> {
        UartDevice {
            state: Cell::new(UartDeviceReceiveState::Idle),
            mux: mux,
            receiver: receiver,
            tx_buffer: TakeCell::empty(),
            transmitting: Cell::new(false),
            rx_buffer: TakeCell::empty(),
            rx_position: Cell::new(0),
            rx_len: Cell::new(0),
            operation: OptionalCell::empty(),
            next: ListLink::empty(),
            rx_client: OptionalCell::empty(),
            tx_client: OptionalCell::empty(),
        }
    }

    /// Must be called right after `static_init!()`.
    pub fn setup(&'a self) {
        self.mux.devices.push_head(self);
    }
}

impl<'a> uart::TransmitClient for UartDevice<'a> {
    fn transmitted_buffer(
        &self,
        tx_buffer: &'static mut [u8],
        tx_len: usize,
        rcode: Result<(), ErrorCode>,
    ) {
        self.tx_client.map(move |client| {
            self.transmitting.set(false);
            client.transmitted_buffer(tx_buffer, tx_len, rcode);
        });
    }

    fn transmitted_word(&self, rcode: Result<(), ErrorCode>) {
        self.tx_client.map(move |client| {
            self.transmitting.set(false);
            client.transmitted_word(rcode);
        });
    }
}
impl<'a> uart::ReceiveClient for UartDevice<'a> {
    fn received_buffer(
        &self,
        rx_buffer: &'static mut [u8],
        rx_len: usize,
        rcode: Result<(), ErrorCode>,
        error: uart::Error,
    ) {
        self.rx_client.map(move |client| {
            self.state.set(UartDeviceReceiveState::Idle);
            client.received_buffer(rx_buffer, rx_len, rcode, error);
        });
    }
}

impl<'a> ListNode<'a, UartDevice<'a>> for UartDevice<'a> {
    fn next(&'a self) -> &'a ListLink<'a, UartDevice<'a>> {
        &self.next
    }
}

impl<'a> uart::Transmit<'a> for UartDevice<'a> {
    fn set_transmit_client(&self, client: &'a dyn uart::TransmitClient) {
        self.tx_client.set(client);
    }

    fn transmit_abort(&self) -> Result<(), ErrorCode> {
        Err(ErrorCode::FAIL)
    }

    /// Transmit data.
    fn transmit_buffer(
        &self,
        tx_data: &'static mut [u8],
        tx_len: usize,
    ) -> Result<(), (ErrorCode, &'static mut [u8])> {
        if self.transmitting.get() {
            Err((ErrorCode::BUSY, tx_data))
        } else {
            self.tx_buffer.replace(tx_data);
            self.transmitting.set(true);
            self.operation.set(Operation::Transmit { len: tx_len });
            self.mux.do_next_op_async();
            Ok(())
        }
    }

    fn transmit_word(&self, word: u32) -> Result<(), ErrorCode> {
        if self.transmitting.get() {
            Err(ErrorCode::BUSY)
        } else {
            self.transmitting.set(true);
            self.operation.set(Operation::TransmitWord { word: word });
            self.mux.do_next_op_async();
            Ok(())
        }
    }
}

impl<'a> uart::Receive<'a> for UartDevice<'a> {
    fn set_receive_client(&self, client: &'a dyn uart::ReceiveClient) {
        self.rx_client.set(client);
    }

    /// Receive data until buffer is full.
    fn receive_buffer(
        &self,
        rx_buffer: &'static mut [u8],
        rx_len: usize,
    ) -> Result<(), (ErrorCode, &'static mut [u8])> {
        if self.rx_buffer.is_some() {
            Err((ErrorCode::BUSY, rx_buffer))
        } else if rx_len > rx_buffer.len() {
            Err((ErrorCode::SIZE, rx_buffer))
        } else {
            self.rx_buffer.replace(rx_buffer);
            self.rx_len.set(rx_len);
            self.rx_position.set(0);
            self.state.set(UartDeviceReceiveState::Idle);
            self.mux.start_receive(rx_len);
            self.state.set(UartDeviceReceiveState::Receiving);
            Ok(())
        }
    }

    // This virtualized device will abort its read: other devices
    // devices will continue with their reads.
    fn receive_abort(&self) -> Result<(), ErrorCode> {
        self.state.set(UartDeviceReceiveState::Aborting);
        let _ = self.mux.uart.receive_abort();
        Err(ErrorCode::BUSY)
    }

    fn receive_word(&self) -> Result<(), ErrorCode> {
        Err(ErrorCode::FAIL)
    }
}

pub static mut WRITE_BUF: [u8; 64] = [0; 64];
pub static mut READ_BUF: [u8; 64] = [0; 64];

impl<'a> Console<'a> {
    pub fn new(
        uart: &'a dyn uart::UartData<'a>,
        tx_buffer: &'static mut [u8],
        rx_buffer: &'static mut [u8],
        grant: Grant<App, 3>,
    ) -> Console<'a> {
        Console {
            uart: uart,
            apps: grant,
            tx_in_progress: OptionalCell::empty(),
            tx_buffer: TakeCell::new(tx_buffer),
            rx_in_progress: OptionalCell::empty(),
            rx_buffer: TakeCell::new(rx_buffer),
        }
    }

    /// Internal helper function for setting up a new send transaction
    fn send_new(&self, app_id: ProcessId, app: &mut App, len: usize) -> Result<(), ErrorCode> {
        app.write_len = cmp::min(len, app.write_buffer.len());
        app.write_remaining = app.write_len;
        self.send(app_id, app);
        Ok(())
    }

    /// Internal helper function for sending data for an existing transaction.
    /// Cannot fail. If can't send now, it will schedule for sending later.
    fn send(&self, app_id: ProcessId, app: &mut App) {
        if self.tx_in_progress.is_none() {
            self.tx_in_progress.set(app_id);
            self.tx_buffer.take().map(|buffer| {
                let len = app.write_buffer.enter(|data| data.len()).unwrap_or(0);
                if app.write_remaining > len {
                    // A slice has changed under us and is now smaller than
                    // what we need to write -- just write what we can.
                    app.write_remaining = len;
                }
                let transaction_len = app
                    .write_buffer
                    .enter(|data| {
                        for (i, c) in data[data.len() - app.write_remaining..data.len()]
                            .iter()
                            .enumerate()
                        {
                            if buffer.len() <= i {
                                return i; // Short circuit on partial send
                            }
                            buffer[i] = c.get();
                        }
                        app.write_remaining
                    })
                    .unwrap_or(0);
                app.write_remaining -= transaction_len;
                let _ = self.uart.transmit_buffer(buffer, transaction_len);
            });
        } else {
            app.pending_write = true;
        }
    }

    /// Internal helper function for starting a receive operation
    fn receive_new(&self, app_id: ProcessId, app: &mut App, len: usize) -> Result<(), ErrorCode> {
        if self.rx_buffer.is_none() {
            // For now, we tolerate only one concurrent receive operation on this console.
            // Competing apps will have to retry until success.
            return Err(ErrorCode::BUSY);
        }

        let read_len = cmp::min(len, app.read_buffer.len());
        if read_len > self.rx_buffer.map_or(0, |buf| buf.len()) {
            // For simplicity, impose a small maximum receive length
            // instead of doing incremental reads
            Err(ErrorCode::INVAL)
        } else {
            // Note: We have ensured above that rx_buffer is present
            app.read_len = read_len;
            self.rx_buffer.take().map(|buffer| {
                self.rx_in_progress.set(app_id);
                let _ = self.uart.receive_buffer(buffer, app.read_len);
            });
            Ok(())
        }
    }
}

impl SyscallDriver for SuperConsole<'_> {
    /// Setup shared buffers.
    ///
    /// ### `allow_num`
    ///
    /// - `1`: Writeable buffer for read buffer
    fn allow_readwrite(
        &self,
        appid: ProcessId,
        allow_num: usize,
        mut slice: ReadWriteProcessBuffer,
    ) -> Result<ReadWriteProcessBuffer, (ReadWriteProcessBuffer, ErrorCode)> {
        let res = match allow_num {
            1 => self
                .apps
                .enter(appid, |app, _| {
                    mem::swap(&mut app.read_buffer, &mut slice);
                })
                .map_err(ErrorCode::from),
            _ => Err(ErrorCode::NOSUPPORT),
        };

        if let Err(e) = res {
            Err((slice, e))
        } else {
            Ok(slice)
        }
    }

    /// Setup shared buffers.
    ///
    /// ### `allow_num`
    ///
    /// - `1`: Readonly buffer for write buffer
    fn allow_readonly(
        &self,
        appid: ProcessId,
        allow_num: usize,
        mut slice: ReadOnlyProcessBuffer,
    ) -> Result<ReadOnlyProcessBuffer, (ReadOnlyProcessBuffer, ErrorCode)> {
        let res = match allow_num {
            1 => self
                .apps
                .enter(appid, |app, _| {
                    mem::swap(&mut app.write_buffer, &mut slice);
                })
                .map_err(ErrorCode::from),
            _ => Err(ErrorCode::NOSUPPORT),
        };

        if let Err(e) = res {
            Err((slice, e))
        } else {
            Ok(slice)
        }
    }

    // Setup callbacks.
    //
    // ### `subscribe_num`
    //
    // - `1`: Write buffer completed callback
    // - `2`: Read buffer completed callback

    /// Initiate serial transfers
    ///
    /// ### `command_num`
    ///
    /// - `0`: Driver check.
    /// - `1`: Transmits a buffer passed via `allow`, up to the length
    ///        passed in `arg1`
    /// - `2`: Receives into a buffer passed via `allow`, up to the length
    ///        passed in `arg1`
    /// - `3`: Cancel any in progress receives and return (via callback)
    ///        what has been received so far.
    fn command(&self, cmd_num: usize, arg1: usize, _: usize, appid: ProcessId) -> CommandReturn {
        let res = self
            .apps
            .enter(appid, |app, _| {
                match cmd_num {
                    0 => Ok(()),
                    1 => {
                        // putstr
                        let len = arg1;
                        self.send_new(appid, app, len)
                    }
                    2 => {
                        // getnstr
                        let len = arg1;
                        self.receive_new(appid, app, len)
                    }
                    3 => {
                        // Abort RX
                        let _ = self.uart.receive_abort();
                        Ok(())
                    }
                    _ => Err(ErrorCode::NOSUPPORT),
                }
            })
            .map_err(ErrorCode::from);
        match res {
            Ok(Ok(())) => CommandReturn::success(),
            Ok(Err(e)) => CommandReturn::failure(e),
            Err(e) => CommandReturn::failure(e),
        }
    }

    fn allocate_grant(&self, processid: ProcessId) -> Result<(), kernel::process::Error> {
        self.apps.enter(processid, |_, _| {})
    }
}

impl uart::TransmitClient for Console<'_> {
    fn transmitted_buffer(
        &self,
        buffer: &'static mut [u8],
        _tx_len: usize,
        _rcode: Result<(), ErrorCode>,
    ) {
        // Either print more from the AppSlice or send a callback to the
        // application.
        self.tx_buffer.replace(buffer);
        self.tx_in_progress.take().map(|appid| {
            self.apps.enter(appid, |app, upcalls| {
                match self.send_continue(appid, app) {
                    Ok(more_to_send) => {
                        if !more_to_send {
                            // Go ahead and signal the application
                            let written = app.write_len;
                            app.write_len = 0;
                            upcalls.schedule_upcall(1, (written, 0, 0)).ok();
                        }
                    }
                    Err(return_code) => {
                        // XXX This shouldn't ever happen?
                        app.write_len = 0;
                        app.write_remaining = 0;
                        app.pending_write = false;
                        upcalls
                            .schedule_upcall(
                                1,
                                (kernel::errorcode::into_statuscode(return_code), 0, 0),
                            )
                            .ok();
                    }
                }
            })
        });

        // If we are not printing more from the current AppSlice,
        // see if any other applications have pending messages.
        if self.tx_in_progress.is_none() {
            for cntr in self.apps.iter() {
                let appid = cntr.processid();
                let started_tx = cntr.enter(|app, upcalls| {
                    if app.pending_write {
                        app.pending_write = false;
                        match self.send_continue(appid, app) {
                            Ok(more_to_send) => more_to_send,
                            Err(return_code) => {
                                // XXX This shouldn't ever happen?
                                app.write_len = 0;
                                app.write_remaining = 0;
                                app.pending_write = false;
                                upcalls
                                    .schedule_upcall(
                                        1,
                                        (kernel::errorcode::into_statuscode(return_code), 0, 0),
                                    )
                                    .ok();
                                false
                            }
                        }
                    } else {
                        false
                    }
                });
                if started_tx {
                    break;
                }
            }
        }
    }
}

impl uart::ReceiveClient for Console<'_> {
    fn received_buffer(
        &self,
        buffer: &'static mut [u8],
        rx_len: usize,
        rcode: Result<(), ErrorCode>,
        error: uart::Error,
    ) {
        self.rx_in_progress
            .take()
            .map(|appid| {
                self.apps
                    .enter(appid, |app, upcalls| {
                        // An iterator over the returned buffer yielding only the first `rx_len`
                        // bytes
                        let rx_buffer = buffer.iter().take(rx_len);
                        match error {
                            uart::Error::None | uart::Error::Aborted => {
                                // Receive some bytes, signal error type and return bytes to process buffer
                                let count = app
                                    .read_buffer
                                    .mut_enter(|data| {
                                        let mut c = 0;
                                        for (a, b) in data.iter().zip(rx_buffer) {
                                            c = c + 1;
                                            a.set(*b);
                                        }
                                        c
                                    })
                                    .unwrap_or(-1);

                                // Make sure we report the same number
                                // of bytes that we actually copied into
                                // the app's buffer. This is defensive:
                                // we shouldn't ever receive more bytes
                                // than will fit in the app buffer since
                                // we use the app_buffer's length when
                                // calling `receive()`. However, a buggy
                                // lower layer could return more bytes
                                // than we asked for, and we don't want
                                // to propagate that length error to
                                // userspace. However, we do return an
                                // error code so that userspace knows
                                // something went wrong.
                                //
                                // If count < 0 this means the buffer
                                // disappeared: return NOMEM.
                                let (ret, received_length) = if count < 0 {
                                    (Err(ErrorCode::NOMEM), 0)
                                } else if rx_len > app.read_buffer.len() {
                                    // Return `SIZE` indicating that
                                    // some received bytes were dropped.
                                    // We report the length that we
                                    // actually copied into the buffer,
                                    // but also indicate that there was
                                    // an issue in the kernel with the
                                    // receive.
                                    (Err(ErrorCode::SIZE), app.read_buffer.len())
                                } else {
                                    // This is the normal and expected
                                    // case.
                                    (rcode, rx_len)
                                };

                                upcalls
                                    .schedule_upcall(
                                        2,
                                        (
                                            kernel::errorcode::into_statuscode(ret),
                                            received_length,
                                            0,
                                        ),
                                    )
                                    .ok();
                            }
                            _ => {
                                // Some UART error occurred
                                upcalls
                                    .schedule_upcall(
                                        2,
                                        (
                                            kernel::errorcode::into_statuscode(Err(
                                                ErrorCode::FAIL,
                                            )),
                                            0,
                                            0,
                                        ),
                                    )
                                    .ok();
                            }
                        }
                    })
                    .unwrap_or_default();
            })
            .unwrap_or_default();

        // Whatever happens, we want to make sure to replace the rx_buffer for future transactions
        self.rx_buffer.replace(buffer);
    }
}
