//! Common CAN interface traits with Tx/Rx split.

#![allow(async_fn_in_trait)]

use core::time::Duration;
use embedded_can::{ExtendedId, StandardId};

/// A CAN identifier (standard 11-bit or extended 29-bit).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Id {
    /// Standard 11-bit identifier.
    Standard(StandardId),
    /// Extended 29-bit identifier.
    Extended(ExtendedId),
}

/// Mask corresponding to a CAN identifier (standard or extended width).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdMask {
    /// Mask for a standard 11-bit identifier.
    Standard(u16),
    /// Mask for an extended 29-bit identifier.
    Extended(u32),
}

/// Filter on a CAN ID with a mask.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IdMaskFilter {
    /// Identifier to match (standard or extended).
    pub id: Id,
    /// Mask to apply; ones are compared, zeros are don't-care.
    pub mask: IdMask,
}

/// Transmit-side frame I/O.
pub trait TxFrameIo {
    /// Frame type.
    type Frame;
    /// Error type.
    type Error;

    /// Blocking send of a frame.
    fn send(&mut self, frame: &Self::Frame) -> Result<(), Self::Error>;

    /// Non-blocking send of a frame.
    fn try_send(&mut self, frame: &Self::Frame) -> Result<(), Self::Error>;

    /// Send a frame with a timeout (when supported by the backend).
    fn send_timeout(&mut self, frame: &Self::Frame, timeout: Duration) -> Result<(), Self::Error>;
}

/// Receive-side frame I/O.
pub trait RxFrameIo {
    /// Frame type.
    type Frame;
    /// Error type.
    type Error;

    /// Blocking receive of a frame.
    fn recv(&mut self) -> Result<Self::Frame, Self::Error>;

    /// Non-blocking receive of a frame.
    fn try_recv(&mut self) -> Result<Self::Frame, Self::Error>;

    /// Receive a frame with a timeout (when supported by the backend).
    fn recv_timeout(&mut self, timeout: Duration) -> Result<Self::Frame, Self::Error>;

    /// Wait until the receive queue is non-empty (sync or async depending on impl).
    fn wait_not_empty(&mut self) -> Result<(), Self::Error>;
}

/// Async transmit-side frame I/O.
pub trait AsyncTxFrameIo {
    /// Frame type.
    type Frame;
    /// Error type.
    type Error;

    /// Asynchronously send a frame.
    async fn send(&mut self, frame: &Self::Frame) -> Result<(), Self::Error>;

    /// Asynchronously send a frame with a timeout (when supported by the backend).
    async fn send_timeout(
        &mut self,
        frame: &Self::Frame,
        timeout: Duration,
    ) -> Result<(), Self::Error>;
}

/// Async receive-side frame I/O.
pub trait AsyncRxFrameIo {
    /// Frame type.
    type Frame;
    /// Error type.
    type Error;

    /// Asynchronously receive a frame.
    async fn recv(&mut self) -> Result<Self::Frame, Self::Error>;

    /// Asynchronously receive a frame with a timeout (when supported by the backend).
    async fn recv_timeout(&mut self, timeout: Duration) -> Result<Self::Frame, Self::Error>;

    /// Asynchronously wait until the receive queue is non-empty.
    async fn wait_not_empty(&mut self) -> Result<(), Self::Error>;
}

/// Convenience marker for types that implement both Tx and Rx for the same frame/error.
pub trait FrameIo:
    TxFrameIo<Frame = <Self as RxFrameIo>::Frame, Error = <Self as RxFrameIo>::Error> + RxFrameIo
{
}

impl<T> FrameIo for T where
    T: TxFrameIo<Frame = <T as RxFrameIo>::Frame, Error = <T as RxFrameIo>::Error> + RxFrameIo
{
}

/// Convenience marker for types that implement both async Tx and async Rx for the same frame/error.
pub trait AsyncFrameIo:
    AsyncTxFrameIo<Frame = <Self as AsyncRxFrameIo>::Frame, Error = <Self as AsyncRxFrameIo>::Error>
    + AsyncRxFrameIo
{
}

impl<T> AsyncFrameIo for T where
    T: AsyncTxFrameIo<Frame = <T as AsyncRxFrameIo>::Frame, Error = <T as AsyncRxFrameIo>::Error>
        + AsyncRxFrameIo
{
}

/// Split a CAN interface into transmit and receive halves.
pub trait SplitTxRx {
    /// Transmit half type.
    type Tx;
    /// Receive half type.
    type Rx;

    /// Split into `(Tx, Rx)` halves.
    fn split(self) -> (Self::Tx, Self::Rx);
}

/// Configure acceptance filters.
pub trait FilterConfig {
    /// Error type.
    type Error;

    /// Type used for modifying filters in-place (hardware bank accessor).
    type FiltersHandle<'a>: 'a
    where
        Self: 'a;

    /// Set filters as a list of ID/mask pairs; implementations may error if limits are exceeded.
    fn set_filters(&mut self, filters: &[IdMaskFilter]) -> Result<(), Self::Error>
    where
        Self: Sized;

    /// Access filter banks through a handle (optional ergonomic API).
    fn modify_filters(&mut self) -> Self::FiltersHandle<'_>;
}

/// Inspect transmit/receive state.
pub trait TxRxState {
    /// Error type.
    type Error;

    /// Check if all transmit mailboxes are idle.
    fn is_transmitter_idle(&self) -> Result<bool, Self::Error>;
}

/// Control blocking vs nonblocking behavior.
pub trait BlockingControl {
    /// Error type.
    type Error;

    /// Globally toggle nonblocking mode.
    fn set_nonblocking(&mut self, on: bool) -> Result<(), Self::Error>;
}

/// Buffered I/O wrapper creation.
pub trait BufferedIo {
    /// Frame type.
    type Frame;
    /// Error type.
    type Error;
    /// Buffered wrapper type.
    type Buffered<'a, const TX: usize, const RX: usize>
    where
        Self: 'a;

    /// Wrap the interface with host-side TX/RX ring buffers.
    fn buffered<'a, const TX: usize, const RX: usize>(
        &'a mut self,
        tx: &'a mut [Self::Frame; TX],
        rx: &'a mut [Self::Frame; RX],
    ) -> Self::Buffered<'a, TX, RX>;
}

/// Constructors/binding helpers.
pub trait BuilderBinding: Sized {
    /// Error type.
    type Error;
    /// Builder type.
    type Builder;

    /// Open/bind by interface name (socketcan-style).
    fn open(name: &str) -> Result<Self, Self::Error>;

    /// Create a builder that can configure before constructing the driver.
    fn builder() -> Self::Builder;
}
