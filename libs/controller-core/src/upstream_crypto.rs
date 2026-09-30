//! Exact, hash-checked upstream identity/cipher code emitted by build.rs.
//! This module is an internal integration seam, not a new cryptographic protocol.
#![allow(dead_code)]

use anyhow::{anyhow, bail};
use bytes::{BufMut, Bytes, BytesMut};
use protobuf::Message as _;
use sodiumoxide::crypto::{
    box_, generichash,
    secretbox::{self, Key, Nonce},
    sign,
};
use std::io::{Error, ErrorKind};

use crate::protos::rendezvous::IdPk;

type ResultType<T> = anyhow::Result<T>;

include!(concat!(env!("OUT_DIR"), "/upstream_crypto.rs"));
