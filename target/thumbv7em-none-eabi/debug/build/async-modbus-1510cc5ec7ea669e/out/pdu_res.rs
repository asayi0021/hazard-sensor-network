use super::Response;
use zerocopy_derive::*;
use zerocopy::big_endian;
///`ReadCoils` PDU (function code `0x01`).
#[derive(Debug, Clone, FromBytes, IntoBytes, Immutable, Unaligned, KnownLayout)]
#[repr(C)]
pub struct ReadCoils<const N: usize> {
    function_code: u8,
    pub(crate) byte_count: u8,
    pub(crate) data: [big_endian::U16; N],
}
impl<const N: usize> ReadCoils<N> {
    ///Creates a new [`ReadCoils`] with default field values.
    pub const fn new() -> Self {
        Self {
            function_code: <Self as crate::Pdu>::FUNCTION_CODE,
            byte_count: 2 * N as u8,
            data: [big_endian::U16::ZERO; N],
        }
    }
    ///Returns a reference to `byte_count`.
    pub const fn byte_count(&self) -> &u8 {
        &self.byte_count
    }
    ///Returns a reference to `data`.
    pub const fn data(&self) -> &[big_endian::U16; N] {
        &self.data
    }
    ///Sets `data`.
    pub const fn set_data(&mut self, new: [big_endian::U16; N]) -> &mut Self {
        self.data = new;
        self
    }
    ///Sets `data`, returning `self`.
    pub const fn with_data(mut self, new: [big_endian::U16; N]) -> Self {
        self.data = new;
        self
    }
    ///Returns a mutable reference to `data`.
    pub const fn data_mut(&mut self) -> &mut [big_endian::U16; N] {
        &mut self.data
    }
}
impl<const N: usize> crate::Pdu for ReadCoils<N> {
    const FUNCTION_CODE: u8 = 1u8;
    const DEFAULT: Self = Self::new();
}
impl<const N: usize> Default for ReadCoils<N> {
    fn default() -> Self {
        crate::Pdu::DEFAULT
    }
}
impl<const N: usize> Response<super::request::ReadCoils> for ReadCoils<N> {
    type Data = [big_endian::U16; N];
    fn matches_request(&self, req: &super::request::ReadCoils) -> bool {
        self.byte_count as u16 == 2 * req.n_registers.get()
    }
    fn into_data(self) -> Self::Data {
        self.data
    }
}
///`ReadDiscreteInputs` PDU (function code `0x02`).
#[derive(Debug, Clone, FromBytes, IntoBytes, Immutable, Unaligned, KnownLayout)]
#[repr(C)]
pub struct ReadDiscreteInputs<const N: usize> {
    function_code: u8,
    pub(crate) byte_count: u8,
    pub(crate) data: [big_endian::U16; N],
}
impl<const N: usize> ReadDiscreteInputs<N> {
    ///Creates a new [`ReadDiscreteInputs`] with default field values.
    pub const fn new() -> Self {
        Self {
            function_code: <Self as crate::Pdu>::FUNCTION_CODE,
            byte_count: 2 * N as u8,
            data: [big_endian::U16::ZERO; N],
        }
    }
    ///Returns a reference to `byte_count`.
    pub const fn byte_count(&self) -> &u8 {
        &self.byte_count
    }
    ///Returns a reference to `data`.
    pub const fn data(&self) -> &[big_endian::U16; N] {
        &self.data
    }
    ///Sets `data`.
    pub const fn set_data(&mut self, new: [big_endian::U16; N]) -> &mut Self {
        self.data = new;
        self
    }
    ///Sets `data`, returning `self`.
    pub const fn with_data(mut self, new: [big_endian::U16; N]) -> Self {
        self.data = new;
        self
    }
    ///Returns a mutable reference to `data`.
    pub const fn data_mut(&mut self) -> &mut [big_endian::U16; N] {
        &mut self.data
    }
}
impl<const N: usize> crate::Pdu for ReadDiscreteInputs<N> {
    const FUNCTION_CODE: u8 = 2u8;
    const DEFAULT: Self = Self::new();
}
impl<const N: usize> Default for ReadDiscreteInputs<N> {
    fn default() -> Self {
        crate::Pdu::DEFAULT
    }
}
impl<const N: usize> Response<super::request::ReadDiscreteInputs>
for ReadDiscreteInputs<N> {
    type Data = [big_endian::U16; N];
    fn matches_request(&self, req: &super::request::ReadDiscreteInputs) -> bool {
        self.byte_count as u16 == 2 * req.n_registers.get()
    }
    fn into_data(self) -> Self::Data {
        self.data
    }
}
///`ReadHoldings` PDU (function code `0x03`).
#[derive(Debug, Clone, FromBytes, IntoBytes, Immutable, Unaligned, KnownLayout)]
#[repr(C)]
pub struct ReadHoldings<const N: usize> {
    function_code: u8,
    pub(crate) byte_count: u8,
    pub(crate) data: [big_endian::U16; N],
}
impl<const N: usize> ReadHoldings<N> {
    ///Creates a new [`ReadHoldings`] with default field values.
    pub const fn new() -> Self {
        Self {
            function_code: <Self as crate::Pdu>::FUNCTION_CODE,
            byte_count: 2 * N as u8,
            data: [big_endian::U16::ZERO; N],
        }
    }
    ///Returns a reference to `byte_count`.
    pub const fn byte_count(&self) -> &u8 {
        &self.byte_count
    }
    ///Returns a reference to `data`.
    pub const fn data(&self) -> &[big_endian::U16; N] {
        &self.data
    }
    ///Sets `data`.
    pub const fn set_data(&mut self, new: [big_endian::U16; N]) -> &mut Self {
        self.data = new;
        self
    }
    ///Sets `data`, returning `self`.
    pub const fn with_data(mut self, new: [big_endian::U16; N]) -> Self {
        self.data = new;
        self
    }
    ///Returns a mutable reference to `data`.
    pub const fn data_mut(&mut self) -> &mut [big_endian::U16; N] {
        &mut self.data
    }
}
impl<const N: usize> crate::Pdu for ReadHoldings<N> {
    const FUNCTION_CODE: u8 = 3u8;
    const DEFAULT: Self = Self::new();
}
impl<const N: usize> Default for ReadHoldings<N> {
    fn default() -> Self {
        crate::Pdu::DEFAULT
    }
}
impl<const N: usize> Response<super::request::ReadHoldings> for ReadHoldings<N> {
    type Data = [big_endian::U16; N];
    fn matches_request(&self, req: &super::request::ReadHoldings) -> bool {
        self.byte_count as u16 == 2 * req.n_registers.get()
    }
    fn into_data(self) -> Self::Data {
        self.data
    }
}
///`ReadInputs` PDU (function code `0x04`).
#[derive(Debug, Clone, FromBytes, IntoBytes, Immutable, Unaligned, KnownLayout)]
#[repr(C)]
pub struct ReadInputs<const N: usize> {
    function_code: u8,
    pub(crate) byte_count: u8,
    pub(crate) data: [big_endian::U16; N],
}
impl<const N: usize> ReadInputs<N> {
    ///Creates a new [`ReadInputs`] with default field values.
    pub const fn new() -> Self {
        Self {
            function_code: <Self as crate::Pdu>::FUNCTION_CODE,
            byte_count: 2 * N as u8,
            data: [big_endian::U16::ZERO; N],
        }
    }
    ///Returns a reference to `byte_count`.
    pub const fn byte_count(&self) -> &u8 {
        &self.byte_count
    }
    ///Returns a reference to `data`.
    pub const fn data(&self) -> &[big_endian::U16; N] {
        &self.data
    }
    ///Sets `data`.
    pub const fn set_data(&mut self, new: [big_endian::U16; N]) -> &mut Self {
        self.data = new;
        self
    }
    ///Sets `data`, returning `self`.
    pub const fn with_data(mut self, new: [big_endian::U16; N]) -> Self {
        self.data = new;
        self
    }
    ///Returns a mutable reference to `data`.
    pub const fn data_mut(&mut self) -> &mut [big_endian::U16; N] {
        &mut self.data
    }
}
impl<const N: usize> crate::Pdu for ReadInputs<N> {
    const FUNCTION_CODE: u8 = 4u8;
    const DEFAULT: Self = Self::new();
}
impl<const N: usize> Default for ReadInputs<N> {
    fn default() -> Self {
        crate::Pdu::DEFAULT
    }
}
impl<const N: usize> Response<super::request::ReadInputs> for ReadInputs<N> {
    type Data = [big_endian::U16; N];
    fn matches_request(&self, req: &super::request::ReadInputs) -> bool {
        self.byte_count as u16 == 2 * req.n_registers.get()
    }
    fn into_data(self) -> Self::Data {
        self.data
    }
}
///`WriteHolding` PDU (function code `0x06`).
#[derive(Debug, Clone, FromBytes, IntoBytes, Immutable, Unaligned, KnownLayout)]
#[repr(C)]
pub struct WriteHolding {
    function_code: u8,
    pub(crate) register: big_endian::U16,
    pub(crate) value: big_endian::U16,
}
impl WriteHolding {
    ///Creates a new [`WriteHolding`] with default field values.
    pub const fn new() -> Self {
        Self {
            function_code: <Self as crate::Pdu>::FUNCTION_CODE,
            register: big_endian::U16::ZERO,
            value: big_endian::U16::ZERO,
        }
    }
    ///Returns a reference to `register`.
    pub const fn register(&self) -> &big_endian::U16 {
        &self.register
    }
    ///Sets `register`.
    pub const fn set_register(&mut self, new: u16) -> &mut Self {
        self.register = big_endian::U16::new(new);
        self
    }
    ///Sets `register`, returning `self`.
    pub const fn with_register(mut self, new: u16) -> Self {
        self.register = big_endian::U16::new(new);
        self
    }
    ///Returns a mutable reference to `register`.
    pub const fn register_mut(&mut self) -> &mut big_endian::U16 {
        &mut self.register
    }
    ///Returns a reference to `value`.
    pub const fn value(&self) -> &big_endian::U16 {
        &self.value
    }
    ///Sets `value`.
    pub const fn set_value(&mut self, new: u16) -> &mut Self {
        self.value = big_endian::U16::new(new);
        self
    }
    ///Sets `value`, returning `self`.
    pub const fn with_value(mut self, new: u16) -> Self {
        self.value = big_endian::U16::new(new);
        self
    }
    ///Returns a mutable reference to `value`.
    pub const fn value_mut(&mut self) -> &mut big_endian::U16 {
        &mut self.value
    }
}
impl crate::Pdu for WriteHolding {
    const FUNCTION_CODE: u8 = 6u8;
    const DEFAULT: Self = Self::new();
}
impl Default for WriteHolding {
    fn default() -> Self {
        crate::Pdu::DEFAULT
    }
}
impl Response<super::request::WriteHolding> for WriteHolding {
    type Data = ();
    fn matches_request(&self, req: &super::request::WriteHolding) -> bool {
        self.register == req.register && self.value == req.value
    }
    fn into_data(self) -> Self::Data {}
}
///`WriteHoldings` PDU (function code `0x10`).
#[derive(Debug, Clone, FromBytes, IntoBytes, Immutable, Unaligned, KnownLayout)]
#[repr(C)]
pub struct WriteHoldings {
    function_code: u8,
    pub(crate) starting_register: big_endian::U16,
    pub(crate) quantity: big_endian::U16,
}
impl WriteHoldings {
    ///Creates a new [`WriteHoldings`] with default field values.
    pub const fn new() -> Self {
        Self {
            function_code: <Self as crate::Pdu>::FUNCTION_CODE,
            starting_register: big_endian::U16::ZERO,
            quantity: big_endian::U16::ZERO,
        }
    }
    ///Returns a reference to `starting_register`.
    pub const fn starting_register(&self) -> &big_endian::U16 {
        &self.starting_register
    }
    ///Sets `starting_register`.
    pub const fn set_starting_register(&mut self, new: u16) -> &mut Self {
        self.starting_register = big_endian::U16::new(new);
        self
    }
    ///Sets `starting_register`, returning `self`.
    pub const fn with_starting_register(mut self, new: u16) -> Self {
        self.starting_register = big_endian::U16::new(new);
        self
    }
    ///Returns a mutable reference to `starting_register`.
    pub const fn starting_register_mut(&mut self) -> &mut big_endian::U16 {
        &mut self.starting_register
    }
    ///Returns a reference to `quantity`.
    pub const fn quantity(&self) -> &big_endian::U16 {
        &self.quantity
    }
    ///Sets `quantity`.
    pub const fn set_quantity(&mut self, new: u16) -> &mut Self {
        self.quantity = big_endian::U16::new(new);
        self
    }
    ///Sets `quantity`, returning `self`.
    pub const fn with_quantity(mut self, new: u16) -> Self {
        self.quantity = big_endian::U16::new(new);
        self
    }
    ///Returns a mutable reference to `quantity`.
    pub const fn quantity_mut(&mut self) -> &mut big_endian::U16 {
        &mut self.quantity
    }
}
impl crate::Pdu for WriteHoldings {
    const FUNCTION_CODE: u8 = 16u8;
    const DEFAULT: Self = Self::new();
}
impl Default for WriteHoldings {
    fn default() -> Self {
        crate::Pdu::DEFAULT
    }
}
impl<const N: usize> Response<super::request::WriteHoldings<N>> for WriteHoldings {
    type Data = ();
    fn matches_request(&self, req: &super::request::WriteHoldings<N>) -> bool {
        self.starting_register == req.starting_register
            && self.quantity.get() == N as u16
    }
    fn into_data(self) -> Self::Data {}
}
