// Copyright (c) Aptos
// SPDX-License-Identifier: Apache-2.0

use move_deps::{
    move_binary_format::errors::{PartialVMError, PartialVMResult},
    move_core_types::vm_status::StatusCode,
    move_vm_runtime::native_functions::{NativeContext, NativeFunction},
    move_vm_types::{
        loaded_data::runtime_types::Type, natives::function::NativeResult, pop_arg, values::Value,
    },
};
use smallvec::smallvec;
use std::{collections::VecDeque, sync::Arc};

/// Abort code when from_bytes fails (0x03 == INVALID_ARGUMENT)
const EFROM_BYTES: u64 = 0x01_0001;

pub fn make_native_from_func<T: std::marker::Send + std::marker::Sync + 'static>(
    gas_params: T,
    func: fn(&T, &mut NativeContext, Vec<Type>, VecDeque<Value>) -> PartialVMResult<NativeResult>,
) -> NativeFunction {
    Arc::new(move |context, ty_args, args| func(&gas_params, context, ty_args, args))
}

/***************************************************************************************************
 * native fun from_bytes
 *
 *   gas cost: base_cost + unit_cost * bytes_len
 *
 **************************************************************************************************/
#[derive(Debug, Clone)]
pub struct FromBytesGasParameters {
    pub base_cost: u64,
    pub unit_cost: u64,
}

fn native_from_bytes(
    gas_params: &FromBytesGasParameters,
    context: &mut NativeContext,
    ty_args: Vec<Type>,
    mut args: VecDeque<Value>,
) -> PartialVMResult<NativeResult> {
    debug_assert_eq!(ty_args.len(), 1);
    debug_assert_eq!(args.len(), 1);

    // TODO(Gas): charge for getting the layout
    let layout = context.type_to_type_layout(&ty_args[0])?.ok_or_else(|| {
        PartialVMError::new(StatusCode::UNKNOWN_INVARIANT_VIOLATION_ERROR).with_message(format!(
            "Failed to get layout of type {:?} -- this should not happen",
            ty_args[0]
        ))
    })?;

    let bytes = pop_arg!(args, Vec<u8>);
    let cost = gas_params.base_cost + gas_params.unit_cost * bytes.len() as u64;
    let val = match Value::simple_deserialize(&bytes, &layout) {
        Some(val) => val,
        None => return Ok(NativeResult::err(cost, EFROM_BYTES)),
    };

    Ok(NativeResult::ok(cost, smallvec![val]))
}

pub fn make_native_from_bytes(gas_params: FromBytesGasParameters) -> NativeFunction {
    Arc::new(move |context, ty_args, args| native_from_bytes(&gas_params, context, ty_args, args))
}

/***************************************************************************************************
 * module
 *
 **************************************************************************************************/
#[derive(Debug, Clone)]
pub struct GasParameters {
    pub from_bytes: FromBytesGasParameters,
}

pub fn make_all(gas_params: GasParameters) -> impl Iterator<Item = (String, NativeFunction)> {
    let natives = [("from_bytes", make_native_from_bytes(gas_params.from_bytes))];

    crate::natives::helpers::make_module_natives(natives)
}
