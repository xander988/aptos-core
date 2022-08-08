// Copyright (c) Aptos
// SPDX-License-Identifier: Apache-2.0

use move_deps::{
    move_binary_format::errors::PartialVMResult,
    move_vm_runtime::native_functions::{NativeContext, NativeFunction},
    move_vm_types::{
        loaded_data::runtime_types::Type, natives::function::NativeResult, pop_arg, values::Value,
    },
};
use smallvec::smallvec;
use std::{collections::VecDeque, convert::TryFrom};
use crate::natives::util::make_native_from_func;

#[derive(Debug, Clone)]
pub struct Ristretto255ScalarIsCanonicalGasParameters {
    pub base_cost: u64,
    pub per_point_deserialize_cost: u64,
}

fn native_ristretto255_scalar_is_canonical(
    gas_params: &Ristretto255ScalarIsCanonicalGasParameters,
    _context: &mut NativeContext,
    _ty_args: Vec<Type>,
    mut arguments: VecDeque<Value>,
) -> PartialVMResult<NativeResult> {
    debug_assert!(_ty_args.is_empty());
    debug_assert!(arguments.len() == 1);

    let mut cost = gas_params.base_cost;

    let bytes = pop_arg!(arguments, Vec<u8>);

    // Length should be exactly 32 bytes
    let bytes_slice = match <[u8; 32]>::try_from(bytes) {
        Ok(slice) => slice,
        Err(_) => {
            return Ok(NativeResult::ok(cost, smallvec![Value::bool(false)]));
        }
    };

    cost += gas_params.per_point_deserialize_cost;

    // This will build a Scalar in-memory and call curve25519-dalek's is_canonical
    match curve25519_dalek::scalar::Scalar::from_canonical_bytes(bytes_slice) {
        Some(_) => Ok(NativeResult::ok(cost, smallvec![Value::bool(true)])),
        None => Ok(NativeResult::ok(cost, smallvec![Value::bool(false)]))
    }
}

#[derive(Debug, Clone)]
pub struct GasParameters {
    pub is_canonical: Ristretto255ScalarIsCanonicalGasParameters,
}

pub fn make_all(gas_params: GasParameters) -> impl Iterator<Item = (String, NativeFunction)> {
    let natives = [
        // BLS over BLS12-381
        (
            "is_canonical",
            make_native_from_func(
                gas_params.is_canonical,
                native_ristretto255_scalar_is_canonical,
            ),
        ),
    ];

    crate::natives::helpers::make_module_natives(natives)
}
