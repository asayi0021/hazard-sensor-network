use super::Response;
use zerocopy_derive::*;
use zerocopy::big_endian;
///`ReadCoils` PDU (function code `0x01`).
#[derive(Debug, Clone, FromBytes, IntoBytes, Immutable, Unaligned, KnownLayout)]
#[repr(C)]
pub struct ReadCoils {
    function_code: u8,
    pub(crate) starting_register: big_endian::U16,
    pub(crate) n_registers: big_endian::U16,
}
impl ReadCoils {
    ///Creates a new [`ReadCoils`] with default field values.
    pub const fn new() -> Self {
        Self {
            function_code: <Self as crate::Pdu>::FUNCTION_CODE,
            starting_register: big_endian::U16::ZERO,
            n_registers: big_endian::U16::ZERO,
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
    ///Returns a reference to `n_registers`.
    pub const fn n_registers(&self) -> &big_endian::U16 {
        &self.n_registers
    }
    ///Sets `n_registers`.
    pub const fn set_n_registers(&mut self, new: u16) -> &mut Self {
        self.n_registers = big_endian::U16::new(new);
        self
    }
    ///Sets `n_registers`, returning `self`.
    pub const fn with_n_registers(mut self, new: u16) -> Self {
        self.n_registers = big_endian::U16::new(new);
        self
    }
    ///Returns a mutable reference to `n_registers`.
    pub const fn n_registers_mut(&mut self) -> &mut big_endian::U16 {
        &mut self.n_registers
    }
}
impl crate::Pdu for ReadCoils {
    const FUNCTION_CODE: u8 = 1u8;
    const DEFAULT: Self = Self::new();
}
impl Default for ReadCoils {
    fn default() -> Self {
        crate::Pdu::DEFAULT
    }
}
///`ReadDiscreteInputs` PDU (function code `0x02`).
#[derive(Debug, Clone, FromBytes, IntoBytes, Immutable, Unaligned, KnownLayout)]
#[repr(C)]
pub struct ReadDiscreteInputs {
    function_code: u8,
    pub(crate) starting_register: big_endian::U16,
    pub(crate) n_registers: big_endian::U16,
}
impl ReadDiscreteInputs {
    ///Creates a new [`ReadDiscreteInputs`] with default field values.
    pub const fn new() -> Self {
        Self {
            function_code: <Self as crate::Pdu>::FUNCTION_CODE,
            starting_register: big_endian::U16::ZERO,
            n_registers: big_endian::U16::ZERO,
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
    ///Returns a reference to `n_registers`.
    pub const fn n_registers(&self) -> &big_endian::U16 {
        &self.n_registers
    }
    ///Sets `n_registers`.
    pub const fn set_n_registers(&mut self, new: u16) -> &mut Self {
        self.n_registers = big_endian::U16::new(new);
        self
    }
    ///Sets `n_registers`, returning `self`.
    pub const fn with_n_registers(mut self, new: u16) -> Self {
        self.n_registers = big_endian::U16::new(new);
        self
    }
    ///Returns a mutable reference to `n_registers`.
    pub const fn n_registers_mut(&mut self) -> &mut big_endian::U16 {
        &mut self.n_registers
    }
}
impl crate::Pdu for ReadDiscreteInputs {
    const FUNCTION_CODE: u8 = 2u8;
    const DEFAULT: Self = Self::new();
}
impl Default for ReadDiscreteInputs {
    fn default() -> Self {
        crate::Pdu::DEFAULT
    }
}
///`ReadHoldings` PDU (function code `0x03`).
#[derive(Debug, Clone, FromBytes, IntoBytes, Immutable, Unaligned, KnownLayout)]
#[repr(C)]
pub struct ReadHoldings {
    function_code: u8,
    pub(crate) starting_register: big_endian::U16,
    pub(crate) n_registers: big_endian::U16,
}
impl ReadHoldings {
    ///Creates a new [`ReadHoldings`] with default field values.
    pub const fn new() -> Self {
        Self {
            function_code: <Self as crate::Pdu>::FUNCTION_CODE,
            starting_register: big_endian::U16::ZERO,
            n_registers: big_endian::U16::ZERO,
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
    ///Returns a reference to `n_registers`.
    pub const fn n_registers(&self) -> &big_endian::U16 {
        &self.n_registers
    }
    ///Sets `n_registers`.
    pub const fn set_n_registers(&mut self, new: u16) -> &mut Self {
        self.n_registers = big_endian::U16::new(new);
        self
    }
    ///Sets `n_registers`, returning `self`.
    pub const fn with_n_registers(mut self, new: u16) -> Self {
        self.n_registers = big_endian::U16::new(new);
        self
    }
    ///Returns a mutable reference to `n_registers`.
    pub const fn n_registers_mut(&mut self) -> &mut big_endian::U16 {
        &mut self.n_registers
    }
}
impl crate::Pdu for ReadHoldings {
    const FUNCTION_CODE: u8 = 3u8;
    const DEFAULT: Self = Self::new();
}
impl Default for ReadHoldings {
    fn default() -> Self {
        crate::Pdu::DEFAULT
    }
}
///`ReadInputs` PDU (function code `0x04`).
#[derive(Debug, Clone, FromBytes, IntoBytes, Immutable, Unaligned, KnownLayout)]
#[repr(C)]
pub struct ReadInputs {
    function_code: u8,
    pub(crate) starting_register: big_endian::U16,
    pub(crate) n_registers: big_endian::U16,
}
impl ReadInputs {
    ///Creates a new [`ReadInputs`] with default field values.
    pub const fn new() -> Self {
        Self {
            function_code: <Self as crate::Pdu>::FUNCTION_CODE,
            starting_register: big_endian::U16::ZERO,
            n_registers: big_endian::U16::ZERO,
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
    ///Returns a reference to `n_registers`.
    pub const fn n_registers(&self) -> &big_endian::U16 {
        &self.n_registers
    }
    ///Sets `n_registers`.
    pub const fn set_n_registers(&mut self, new: u16) -> &mut Self {
        self.n_registers = big_endian::U16::new(new);
        self
    }
    ///Sets `n_registers`, returning `self`.
    pub const fn with_n_registers(mut self, new: u16) -> Self {
        self.n_registers = big_endian::U16::new(new);
        self
    }
    ///Returns a mutable reference to `n_registers`.
    pub const fn n_registers_mut(&mut self) -> &mut big_endian::U16 {
        &mut self.n_registers
    }
}
impl crate::Pdu for ReadInputs {
    const FUNCTION_CODE: u8 = 4u8;
    const DEFAULT: Self = Self::new();
}
impl Default for ReadInputs {
    fn default() -> Self {
        crate::Pdu::DEFAULT
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
///`WriteHoldings` PDU (function code `0x10`).
#[derive(Debug, Clone, FromBytes, IntoBytes, Immutable, Unaligned, KnownLayout)]
#[repr(C)]
pub struct WriteHoldings<const N: usize> {
    function_code: u8,
    pub(crate) starting_register: big_endian::U16,
    pub(crate) n_registers: big_endian::U16,
    pub(crate) data_bytes: u8,
    pub(crate) data: [big_endian::U16; N],
}
impl<const N: usize> WriteHoldings<N> {
    ///Creates a new [`WriteHoldings`] with default field values.
    pub const fn new() -> Self {
        Self {
            function_code: <Self as crate::Pdu>::FUNCTION_CODE,
            starting_register: big_endian::U16::ZERO,
            n_registers: big_endian::U16::new(N as u16),
            data_bytes: 2 * N as u8,
            data: [big_endian::U16::ZERO; N],
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
    ///Returns a reference to `n_registers`.
    pub const fn n_registers(&self) -> &big_endian::U16 {
        &self.n_registers
    }
    ///Returns a reference to `data_bytes`.
    pub const fn data_bytes(&self) -> &u8 {
        &self.data_bytes
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
impl<const N: usize> crate::Pdu for WriteHoldings<N> {
    const FUNCTION_CODE: u8 = 16u8;
    const DEFAULT: Self = Self::new();
}
impl<const N: usize> Default for WriteHoldings<N> {
    fn default() -> Self {
        crate::Pdu::DEFAULT
    }
}
