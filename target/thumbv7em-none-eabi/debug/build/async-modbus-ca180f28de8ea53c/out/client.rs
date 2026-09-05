use zerocopy::big_endian;
use embedded_io_async::{Read, Write};
use super::*;
use crate::{
    pdu::{request, response},
    Frame,
};
///Executes a Modbus `ReadCoils` request (function code `0x01`).
pub async fn read_coils_with<const N: usize, E: embedded_io_async::Error>(
    mut serial: impl Read<Error = E> + Write<Error = E>,
    req: &Frame<request::ReadCoils>,
) -> Result<[big_endian::U16; N], Error<E>> {
    write_frame(&mut serial, req).await.map_err(Error::Io)?;
    let res: Frame<response::ReadCoils<N>> = read_frame(&mut serial).await?;
    Ok(res.into_data(req)?)
}
///Executes a Modbus `ReadCoils` request (function code `0x01`).
pub async fn read_coils<const N: usize, E: embedded_io_async::Error>(
    serial: impl Read<Error = E> + Write<Error = E>,
    address: u8,
    starting_register: u16,
) -> Result<[big_endian::U16; N], Error<E>> {
    let mut req = Frame::<request::ReadCoils>::builder(address);
    req.pdu_mut().set_starting_register(starting_register);
    req.pdu_mut().set_n_registers(N as u16);
    let req = req.build_ref();
    read_coils_with(serial, req).await
}
///Executes a Modbus `ReadDiscreteInputs` request (function code `0x02`).
pub async fn read_discrete_inputs_with<const N: usize, E: embedded_io_async::Error>(
    mut serial: impl Read<Error = E> + Write<Error = E>,
    req: &Frame<request::ReadDiscreteInputs>,
) -> Result<[big_endian::U16; N], Error<E>> {
    write_frame(&mut serial, req).await.map_err(Error::Io)?;
    let res: Frame<response::ReadDiscreteInputs<N>> = read_frame(&mut serial).await?;
    Ok(res.into_data(req)?)
}
///Executes a Modbus `ReadDiscreteInputs` request (function code `0x02`).
pub async fn read_discrete_inputs<const N: usize, E: embedded_io_async::Error>(
    serial: impl Read<Error = E> + Write<Error = E>,
    address: u8,
    starting_register: u16,
) -> Result<[big_endian::U16; N], Error<E>> {
    let mut req = Frame::<request::ReadDiscreteInputs>::builder(address);
    req.pdu_mut().set_starting_register(starting_register);
    req.pdu_mut().set_n_registers(N as u16);
    let req = req.build_ref();
    read_discrete_inputs_with(serial, req).await
}
///Executes a Modbus `ReadHoldings` request (function code `0x03`).
pub async fn read_holdings_with<const N: usize, E: embedded_io_async::Error>(
    mut serial: impl Read<Error = E> + Write<Error = E>,
    req: &Frame<request::ReadHoldings>,
) -> Result<[big_endian::U16; N], Error<E>> {
    write_frame(&mut serial, req).await.map_err(Error::Io)?;
    let res: Frame<response::ReadHoldings<N>> = read_frame(&mut serial).await?;
    Ok(res.into_data(req)?)
}
///Executes a Modbus `ReadHoldings` request (function code `0x03`).
pub async fn read_holdings<const N: usize, E: embedded_io_async::Error>(
    serial: impl Read<Error = E> + Write<Error = E>,
    address: u8,
    starting_register: u16,
) -> Result<[big_endian::U16; N], Error<E>> {
    let mut req = Frame::<request::ReadHoldings>::builder(address);
    req.pdu_mut().set_starting_register(starting_register);
    req.pdu_mut().set_n_registers(N as u16);
    let req = req.build_ref();
    read_holdings_with(serial, req).await
}
///Executes a Modbus `ReadInputs` request (function code `0x04`).
pub async fn read_inputs_with<const N: usize, E: embedded_io_async::Error>(
    mut serial: impl Read<Error = E> + Write<Error = E>,
    req: &Frame<request::ReadInputs>,
) -> Result<[big_endian::U16; N], Error<E>> {
    write_frame(&mut serial, req).await.map_err(Error::Io)?;
    let res: Frame<response::ReadInputs<N>> = read_frame(&mut serial).await?;
    Ok(res.into_data(req)?)
}
///Executes a Modbus `ReadInputs` request (function code `0x04`).
pub async fn read_inputs<const N: usize, E: embedded_io_async::Error>(
    serial: impl Read<Error = E> + Write<Error = E>,
    address: u8,
    starting_register: u16,
) -> Result<[big_endian::U16; N], Error<E>> {
    let mut req = Frame::<request::ReadInputs>::builder(address);
    req.pdu_mut().set_starting_register(starting_register);
    req.pdu_mut().set_n_registers(N as u16);
    let req = req.build_ref();
    read_inputs_with(serial, req).await
}
///Executes a Modbus `WriteHolding` request (function code `0x06`).
pub async fn write_holding_with<E: embedded_io_async::Error>(
    mut serial: impl Read<Error = E> + Write<Error = E>,
    req: &Frame<request::WriteHolding>,
) -> Result<(), Error<E>> {
    write_frame(&mut serial, req).await.map_err(Error::Io)?;
    let res: Frame<response::WriteHolding> = read_frame(&mut serial).await?;
    Ok(res.into_data(req)?)
}
///Executes a Modbus `WriteHolding` request (function code `0x06`).
pub async fn write_holding<E: embedded_io_async::Error>(
    serial: impl Read<Error = E> + Write<Error = E>,
    address: u8,
    register: u16,
    value: u16,
) -> Result<(), Error<E>> {
    let mut req = Frame::<request::WriteHolding>::builder(address);
    req.pdu_mut().set_register(register);
    req.pdu_mut().set_value(value);
    let req = req.build_ref();
    write_holding_with(serial, req).await
}
///Executes a Modbus `WriteHoldings` request (function code `0x10`).
pub async fn write_holdings_with<const N: usize, E: embedded_io_async::Error>(
    mut serial: impl Read<Error = E> + Write<Error = E>,
    req: &Frame<request::WriteHoldings<N>>,
) -> Result<(), Error<E>> {
    write_frame(&mut serial, req).await.map_err(Error::Io)?;
    let res: Frame<response::WriteHoldings> = read_frame(&mut serial).await?;
    Ok(res.into_data(req)?)
}
///Executes a Modbus `WriteHoldings` request (function code `0x10`).
pub async fn write_holdings<const N: usize, E: embedded_io_async::Error>(
    serial: impl Read<Error = E> + Write<Error = E>,
    address: u8,
    starting_register: u16,
    data: [big_endian::U16; N],
) -> Result<(), Error<E>> {
    let mut req = Frame::<request::WriteHoldings<N>>::builder(address);
    req.pdu_mut().set_starting_register(starting_register);
    req.pdu_mut().set_data(data);
    let req = req.build_ref();
    write_holdings_with(serial, req).await
}
