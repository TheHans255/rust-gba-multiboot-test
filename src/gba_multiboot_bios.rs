use core::arch::asm;

#[repr(C)]
pub struct MultibootParameter {
    pub _padding0: [u8; 20],
    pub handshake_data: u8,
    pub _padding1: [u8; 4],
    pub client_data: [u8; 3],
    pub palette_data: u8,
    pub _padding2: u8,
    pub client_bit: u8,
    pub _padding3: u8,
    pub boot_srcp: * const u8,
    pub boot_endp: * const u8
}

impl Default for MultibootParameter {
    fn default() -> Self {
        Self {
            _padding0: [0; 20],
            handshake_data: 0,
            _padding1: [0; 4],
            client_data: [0xff; 3],
            palette_data: 0x81,
            _padding2: 0,
            client_bit: 0,
            _padding3: 0,
            boot_srcp:core::ptr::null(),
            boot_endp:core::ptr::null(),
        }
    }
}

#[derive(Copy, Clone)]
pub enum MultibootTransferMode {
    Normal = 0,
    MultiPlay = 1,
    NormalUnstable = 2
}

#[inline]
#[instruction_set(arm::t32)]
pub unsafe fn Multiboot(params: * const MultibootParameter, mode: MultibootTransferMode) -> Result<(),()> {
    let result: u32;
    asm!("swi 0x25",
        inlateout("r0") params as u32 => result,
        inlateout("r1") mode as u8 => _,
        options(nomem, nostack)
    );
    if result == 0 { Ok(()) } else { Err(()) }
}