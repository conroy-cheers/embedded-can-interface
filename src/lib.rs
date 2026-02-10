//! `embedded-can-interface`: HAL-style I/O traits for CAN controllers.
//!
//! This crate provides a small, `no_std`-friendly set of traits that describe the *shape* of a CAN
//! interface, without committing to any particular runtime (blocking vs async), driver model, or
//! buffer ownership.
//!
//! It is intended to sit between:
//! - a concrete driver implementation (SocketCAN, bxCAN, MCP2515, a simulator, …), and
//! - protocol layers that need to send/receive CAN frames (e.g. ISO-TP, UDS, J1939, proprietary
//!   application protocols).
//!
//! The core idea is to support both “monolithic” CAN devices and split transmit/receive halves.
//! Many protocol stacks are easier to integrate with when Tx and Rx are separate objects with
//! independent borrowing, so this crate explicitly models that pattern.
//!
//! # What this crate does (and does not) do
//! - ✅ Defines traits for sending/receiving frames, configuring acceptance filters, and optional
//!   driver controls (nonblocking toggle, TX-idle query, buffering wrapper, builder/binding).
//! - ✅ Provides small helper types for common ID/mask filter patterns.
//! - ❌ Does not define an error model (e.g. “would block” vs “bus off”); that remains driver-
//!   specific.
//! - ❌ Does not define a frame type; you use a type implementing [`embedded_can::Frame`].
//!
//! # Quick start
//! Most code consumes *traits*:
//!
//! - If you need only transmit: [`TxFrameIo`]
//! - If you need only receive: [`RxFrameIo`]
//! - If you need both (single object): [`FrameIo`]
//! - If you use a split design: [`SplitTxRx`] to obtain halves
//!
//! ## Blocking example (conceptual)
//! ```rust,ignore
//! use embedded_can_interface::{RxFrameIo, TxFrameIo};
//!
//! fn ping<T>(io: &mut T, frame: &T::Frame) -> Result<T::Frame, T::Error>
//! where
//!     T: TxFrameIo + RxFrameIo<Frame = <T as TxFrameIo>::Frame, Error = <T as TxFrameIo>::Error>,
//! {
//!     io.send(frame)?;
//!     io.recv()
//! }
//! ```
//!
//! ## Async example (conceptual)
//! ```rust,ignore
//! use embedded_can_interface::{AsyncRxFrameIo, AsyncTxFrameIo};
//! use core::time::Duration;
//!
//! async fn ping<T>(io: &mut T, frame: &T::Frame) -> Result<T::Frame, T::Error>
//! where
//!     T: AsyncTxFrameIo + AsyncRxFrameIo<Frame = <T as AsyncTxFrameIo>::Frame, Error = <T as AsyncTxFrameIo>::Error>,
//! {
//!     io.send_timeout(frame, Duration::from_millis(10)).await?;
//!     io.recv_timeout(Duration::from_millis(10)).await
//! }
//! ```
//!
//! # Design notes
//! - `try_*` methods are “non-blocking” in the sense that they should return quickly. This crate
//!   does not prescribe *how* a driver reports “no data available”; many backends use a dedicated
//!   error variant (e.g. `nb::Error::WouldBlock`).
//! - Timeouts are expressed as [`core::time::Duration`]. Implementations may approximate or ignore
//!   timeouts if the underlying platform cannot support them.
//! - `async fn` in traits is permitted via `#![allow(async_fn_in_trait)]` to keep the interface
//!   ergonomic for consumers; concrete drivers can still choose how to implement async operations.

#![no_std]
#![allow(async_fn_in_trait)]

use core::time::Duration;
use embedded_can::{ExtendedId, StandardId};

/// A CAN identifier (standard 11-bit or extended 29-bit).
///
/// Many embedded CAN HALs want to stay `no_std` and avoid allocating or storing extra metadata.
/// This enum is a lightweight wrapper around the ID types provided by [`embedded_can`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Id {
    /// Standard 11-bit identifier.
    Standard(StandardId),
    /// Extended 29-bit identifier.
    Extended(ExtendedId),
}

/// Bitmask corresponding to a CAN identifier (standard or extended width).
///
/// This is typically used for acceptance filtering: a frame is accepted when the masked bits match.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdMask {
    /// Mask for a standard 11-bit identifier.
    Standard(u16),
    /// Mask for an extended 29-bit identifier.
    Extended(u32),
}

/// Acceptance filter: match an [`Id`] against an [`IdMask`].
///
/// Common semantics are:
/// - “mask bit = 1” means “compare this bit”
/// - “mask bit = 0” means “don’t care”
///
/// Exact matching rules and hardware limits are driver-specific.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IdMaskFilter {
    /// Identifier to match (standard or extended).
    pub id: Id,
    /// Mask to apply; ones are compared, zeros are don't-care.
    pub mask: IdMask,
}

/// Transmit-side (blocking) CAN frame I/O.
///
/// This is the minimal interface a protocol needs to *send* frames. You can implement it for a
/// full CAN controller type, or for a dedicated “TX half” returned by [`SplitTxRx`].
pub trait TxFrameIo {
    /// The CAN frame type.
    ///
    /// Most implementations use a concrete frame type from a driver crate; it must represent a CAN
    /// frame (including ID, DLC, payload) and is typically also an [`embedded_can::Frame`].
    type Frame;
    /// Error returned by the driver implementation.
    type Error;

    /// Send a frame, blocking until it is accepted by the driver.
    fn send(&mut self, frame: &Self::Frame) -> Result<(), Self::Error>;

    /// Attempt to send a frame without blocking.
    ///
    /// When the driver cannot accept a frame immediately (e.g. no TX mailbox), implementations
    /// typically return an error such as `nb::Error::WouldBlock`.
    fn try_send(&mut self, frame: &Self::Frame) -> Result<(), Self::Error>;

    /// Send a frame, waiting up to `timeout` for the driver to accept it.
    ///
    /// Implementations that cannot support timeouts may treat this as [`TxFrameIo::send`].
    fn send_timeout(&mut self, frame: &Self::Frame, timeout: Duration) -> Result<(), Self::Error>;
}

/// Receive-side (blocking) CAN frame I/O.
///
/// This is the minimal interface a protocol needs to *receive* frames. You can implement it for a
/// full CAN controller type, or for a dedicated “RX half” returned by [`SplitTxRx`].
pub trait RxFrameIo {
    /// The CAN frame type.
    type Frame;
    /// Error returned by the driver implementation.
    type Error;

    /// Receive a frame, blocking until one is available.
    fn recv(&mut self) -> Result<Self::Frame, Self::Error>;

    /// Attempt to receive a frame without blocking.
    ///
    /// When no frame is available, implementations typically return an error such as
    /// `nb::Error::WouldBlock`.
    fn try_recv(&mut self) -> Result<Self::Frame, Self::Error>;

    /// Receive a frame, waiting up to `timeout`.
    ///
    /// Implementations that cannot support timeouts may treat this as [`RxFrameIo::recv`].
    fn recv_timeout(&mut self, timeout: Duration) -> Result<Self::Frame, Self::Error>;

    /// Wait until the receive queue is non-empty.
    ///
    /// This can be used by polling-style protocols to avoid busy loops.
    fn wait_not_empty(&mut self) -> Result<(), Self::Error>;
}

/// Transmit-side (async) CAN frame I/O.
///
/// This is the async equivalent of [`TxFrameIo`]. It is intentionally small: protocol layers that
/// need async I/O can build on top of it without depending on a specific async runtime.
pub trait AsyncTxFrameIo {
    /// The CAN frame type.
    type Frame;
    /// Error returned by the driver implementation.
    type Error;

    /// Send a frame asynchronously.
    async fn send(&mut self, frame: &Self::Frame) -> Result<(), Self::Error>;

    /// Send a frame asynchronously, waiting up to `timeout`.
    ///
    /// Implementations that cannot support timeouts may treat this as [`AsyncTxFrameIo::send`].
    async fn send_timeout(
        &mut self,
        frame: &Self::Frame,
        timeout: Duration,
    ) -> Result<(), Self::Error>;
}

/// Receive-side (async) CAN frame I/O.
///
/// This is the async equivalent of [`RxFrameIo`].
pub trait AsyncRxFrameIo {
    /// The CAN frame type.
    type Frame;
    /// Error returned by the driver implementation.
    type Error;

    /// Receive a frame asynchronously.
    async fn recv(&mut self) -> Result<Self::Frame, Self::Error>;

    /// Receive a frame asynchronously, waiting up to `timeout`.
    ///
    /// Implementations that cannot support timeouts may treat this as [`AsyncRxFrameIo::recv`].
    async fn recv_timeout(&mut self, timeout: Duration) -> Result<Self::Frame, Self::Error>;

    /// Asynchronously wait until the receive queue is non-empty.
    async fn wait_not_empty(&mut self) -> Result<(), Self::Error>;
}

/// Convenience marker for types that implement both [`TxFrameIo`] and [`RxFrameIo`] using the same
/// frame and error types.
///
/// This is a *marker trait* only; it has no methods and exists to reduce boilerplate in bounds.
pub trait FrameIo:
    TxFrameIo<Frame = <Self as RxFrameIo>::Frame, Error = <Self as RxFrameIo>::Error> + RxFrameIo
{
}

impl<T> FrameIo for T where
    T: TxFrameIo<Frame = <T as RxFrameIo>::Frame, Error = <T as RxFrameIo>::Error> + RxFrameIo
{
}

/// Convenience marker for types that implement both [`AsyncTxFrameIo`] and [`AsyncRxFrameIo`] using
/// the same frame and error types.
///
/// This is a *marker trait* only; it has no methods and exists to reduce boilerplate in bounds.
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
///
/// This trait is usually implemented for a concrete CAN driver type that internally owns shared
/// state, returning lightweight wrapper types (`Tx`, `Rx`) that can be borrowed independently.
pub trait SplitTxRx {
    /// Transmit half type.
    ///
    /// This typically implements [`TxFrameIo`] and/or [`AsyncTxFrameIo`].
    type Tx;
    /// Receive half type.
    ///
    /// This typically implements [`RxFrameIo`] and/or [`AsyncRxFrameIo`].
    type Rx;

    /// Split into `(Tx, Rx)` halves.
    fn split(self) -> (Self::Tx, Self::Rx);
}

/// Configure acceptance filters (aka “hardware filtering”).
///
/// CAN controllers often provide a fixed number of acceptance filter “banks”. Protocol layers may
/// want to install filters to reduce host-side work.
pub trait FilterConfig {
    /// Error returned by the driver implementation.
    type Error;

    /// Handle used for modifying filters in-place (e.g. a bank accessor).
    ///
    /// Some drivers expose more complex configuration operations than “set this list”. A borrowed
    /// handle enables in-place modifications without forcing a particular API shape.
    type FiltersHandle<'a>: 'a
    where
        Self: 'a;

    /// Replace the current filter configuration with a list of ID/mask pairs.
    ///
    /// Implementations may error if:
    /// - the number of filters exceeds hardware limits, or
    /// - a filter is not representable in hardware (e.g. mask width mismatch).
    fn set_filters(&mut self, filters: &[IdMaskFilter]) -> Result<(), Self::Error>
    where
        Self: Sized;

    /// Access filter banks through a handle (optional ergonomic API).
    fn modify_filters(&mut self) -> Self::FiltersHandle<'_>;
}

/// Inspect driver state related to transmit/receive operation.
pub trait TxRxState {
    /// Error returned by the driver implementation.
    type Error;

    /// Returns `true` when the transmitter is idle (no frames pending).
    ///
    /// The exact definition of “idle” depends on the hardware/driver (e.g. all mailboxes empty).
    fn is_transmitter_idle(&self) -> Result<bool, Self::Error>;
}

/// Control blocking vs nonblocking behavior.
///
/// Some drivers can be configured globally to make “blocking” operations return immediately.
pub trait BlockingControl {
    /// Error returned by the driver implementation.
    type Error;

    /// Globally toggle nonblocking mode.
    ///
    /// When `on` is true, methods like [`TxFrameIo::send`] and [`RxFrameIo::recv`] may return early
    /// with a “would block” error.
    fn set_nonblocking(&mut self, on: bool) -> Result<(), Self::Error>;
}

/// Buffered I/O wrapper creation.
///
/// This trait is for drivers that support adding host-side ring buffers around an underlying CAN
/// device. This is common when the hardware has small mailboxes, but the host wants to smooth bursty
/// traffic.
pub trait BufferedIo {
    /// The CAN frame type.
    type Frame;
    /// Error returned by the driver implementation (typically forwarded from the underlying device).
    type Error;
    /// Buffered wrapper type.
    ///
    /// The wrapper usually borrows the driver and user-provided storage.
    type Buffered<'a, const TX: usize, const RX: usize>
    where
        Self: 'a;

    /// Wrap the interface with host-side TX/RX ring buffers.
    ///
    /// The backing storage is provided by the caller to avoid allocation and to make buffer sizes
    /// explicit in types.
    fn buffered<'a, const TX: usize, const RX: usize>(
        &'a mut self,
        tx: &'a mut [Self::Frame; TX],
        rx: &'a mut [Self::Frame; RX],
    ) -> Self::Buffered<'a, TX, RX>;
}

/// Constructors/binding helpers.
///
/// This is an optional trait for backends that can be opened by name (e.g. `can0`) or configured via
/// a builder pattern.
pub trait BuilderBinding: Sized {
    /// Error returned by the driver implementation.
    type Error;
    /// Builder type used to configure before constructing the driver.
    type Builder;

    /// Open/bind by interface name (SocketCAN-style).
    fn open(name: &str) -> Result<Self, Self::Error>;

    /// Create a builder that can configure before constructing the driver.
    fn builder() -> Self::Builder;
}
