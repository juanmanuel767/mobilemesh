// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (c) 2026 Juan Manuel Peralta Chacon (GitHub: juanmanuel767)
//! Programas de servidor de MobileMesh (licencia AGPL-3.0): el relé, el puente para navegadores
//! (WebSocket) y el candado (wss). Las apps no usan nada de aquí: usan `mobilemesh-core` (MIT).

pub mod rele;
pub mod tls;
pub mod websocket;

pub use rele::Relay;
