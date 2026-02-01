# embedded-can-interface

Small `no_std`-friendly interface traits for CAN drivers and protocol layers.

This crate defines:
- Blocking and async Tx/Rx traits (`TxFrameIo`, `RxFrameIo`, `AsyncTxFrameIo`, `AsyncRxFrameIo`)
- Optional split-halves support (`SplitTxRx`)
- Optional driver capabilities (filters, buffering, builder/binding)
