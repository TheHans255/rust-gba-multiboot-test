// TODO: Write the actual GBA Multiboot establishment code here.
// This includes the protocol up to and including calling SWI 0x25.
// You may also want a test image you can use this on, as well
// as some debugging information you can export

// TODO: Write or use a few error types signaling the following
// error conditions:
// - Syscall failure
// - Multiboot payload not large enough
// - No other systems connected
// - Other systems did not respond as expected

use crate::gba_multiboot_bios::*;
use voladdress::{ VolAddress, Safe };
use gba::mmio_addresses::{ RCNT, SIOCNT };

const CARTRIDGE_HEADER_LENGTH: usize = 0xc0;

const SIOMLT_SEND: VolAddress<u16, Safe, Safe> = unsafe{ VolAddress::new(0x400_012A) };
const SIOMULTI0: VolAddress<u16, Safe, Safe> = unsafe{ VolAddress::new(0x400_0120) };
const SIOMULTI1: VolAddress<u16, Safe, Safe> = unsafe{ VolAddress::new(0x400_0122) };
const SIOMULTI2: VolAddress<u16, Safe, Safe> = unsafe{ VolAddress::new(0x400_0124) };
const SIOMULTI3: VolAddress<u16, Safe, Safe> = unsafe{ VolAddress::new(0x400_0126) };

#[derive(Copy, Clone, PartialEq, Eq)]
struct ExchangeUnit {
    command: u8,
    data: u8
}
impl ExchangeUnit {
    fn new(command: u8, data: u8) -> Self{
        Self {command, data }
    }
}
impl From<u16> for ExchangeUnit {
    fn from(x: u16) -> Self {
        Self {
            command: (x >> 8) as u8,
            data: x as u8
        }
    }
}
impl From<ExchangeUnit> for u16 {
    fn from(u: ExchangeUnit) -> Self {
        ((u.command as u16) << 8) + (u.data as u16)
    }
}


/// Exchange a 16-bit unit of data with
/// up to 3 connected clients
fn exchange(send_unit: ExchangeUnit) -> [ExchangeUnit; 3] {
    SIOMLT_SEND.write(send_unit.into());
    SIOCNT.write(SIOCNT.read() | 0x0080);
    for i in 0..0x1000 {
        let busy_bit = SIOCNT.read() & 0x0080;
        if busy_bit == 0 {
            gba::debug!("Exchange completed after {} cycles", i);
            break;
        }
    }
    let result1 = SIOMULTI1.read();
    let result2 = SIOMULTI2.read();
    let result3 = SIOMULTI3.read();
    [result1.into(), result2.into(), result3.into()]
}


/// Initialize a multiboot session with all connected GBAs.
/// Takes a slice pointing to the complete multiboot payload,
/// including the cartridge and multiboot header
pub fn start_multiboot(multiboot_image: &[u8], palette_data: u8) -> Result<(),u16> {
    let payload_length = multiboot_image.len();
    if (payload_length % 0x10) != 0
        || payload_length < 0x100 + CARTRIDGE_HEADER_LENGTH
        || payload_length > 0x3ffff {
            gba::error!("Payload not correct size");
            return Err(0b0_00111_00111_00111)
        }
    gba::info!("Verified payload size at {}", payload_length);

    let mut multiboot_args: MultibootParameter = MultibootParameter::default();
    multiboot_args.palette_data = palette_data;
    unsafe {
        multiboot_args.boot_srcp = core::ptr::addr_of!(multiboot_image[0])
            .offset(CARTRIDGE_HEADER_LENGTH.try_into().unwrap());
        multiboot_args.boot_endp = core::ptr::addr_of!(multiboot_image[0])
            .offset(multiboot_image.len().try_into().unwrap());
    };
    gba::info!("Setup multiboot args");

    // 2. Initiate multiplayer communication, setting RCNT and SIOCNT registers appropriately
    RCNT.write(0);
    SIOCNT.write(0b0010_0001_0000_0011);
    {
        let s = SIOCNT.read();
        if s & 0x0008 == 0 {
            gba::error!("Bad connection");
            return Err(0b0_00000_00000_11111)
        }
        if s & 0x0004 == 1 {
            gba::error!("We're not the parent");
            return Err(0b0_11111_00000_00111)
        }
    }
    gba::info!("Initialized multiplayer session");
    exchange(0x6200.into());
    gba::info!("Sent first byte");
    
    // 3. Send 0x6200 up to 16 times until all clients respond with either 0x720x or 0xffff
    let mut init_response: Option<[ExchangeUnit; 3]> = None;
    'init_send_loop:
    for _i in 0..15 {
        let response = exchange(0x6200.into());
        for j in 0..response.len() {
            let unit = response[j];
            if unit.command != 0xff && (unit.command != 0x72 || (unit.data as usize) != 0x02 << j) {
                gba::info!("Client {} sent command={},data={}, retrying send", j, unit.command, unit.data);
                continue 'init_send_loop;
            }
        }
        init_response = Some(response);
        for unit in response {
            if unit.command == 0x72 {
                break 'init_send_loop;
            }
        }
    }
    if init_response.is_none() {
        gba::error!("Could not get responses");
        return Err(0b0_00111_00000_11111);
    }
    gba::info!("Established multiboot send session");

    // 4. Fill in client_bit with the clients detected. Send the word 0x610y with those set bits
    let mut clients_present: [bool; 3] = [false, false, false];
    for i in 0..init_response.unwrap().len() {
        let unit = (init_response.unwrap())[i];
        gba::info!("Command from client {} was {}", i, unit.command);
        if unit.command == 0x72 {
            clients_present[i] = true;
            multiboot_args.client_bit |= 0x02 << i
        }
    }
    if multiboot_args.client_bit == 0 {
        gba::error!("No other GBAs");
        return Err(0b0_00000_00111_00111);
    }
    gba::info!("Detected clients: {} {} {}", clients_present[0], clients_present[1], clients_present[2]);
    exchange(ExchangeUnit::new(0x61, multiboot_args.client_bit as u8));
    gba::info!("Sent client data to other GBAs");

    // 5. Send the cartridge header, 16 bits at a time. The clients will respond with
    //    0xNN0x, where NN is the number of words remaining and X is the client number
    for i in (0..CARTRIDGE_HEADER_LENGTH).step_by(2) {
        // NOTE: Assuming we want little-endian
        gba::info!("Sending byte {} of cartridge header", i);
        let command = multiboot_image[i + 1];
        let data = multiboot_image[i];
        gba::info!("Sending bytes {} {}", command, data);
        let cartridge_response = exchange(
            ExchangeUnit::new(command, data)
        );
        gba::info!("Bytes sent, reading expected commands");
        let expected_command: u8 = ((CARTRIDGE_HEADER_LENGTH - i) / 2).try_into().unwrap();
        for j in 0..cartridge_response.len() {
            let unit = cartridge_response[j];
            let got_expected = if clients_present[j] {
                unit.command == expected_command && (unit.data as usize) == 0x02 << j
            } else {
                unit.command == 0xff && unit.data == 0xff
            };
            if !got_expected {
                gba::error!("Expected command {} from client {}, got {}", expected_command, j, unit.command);
                return Err(0b0_00000_11111_11111)
            }
        }
        gba::info!("Sent byte {} of cartridge header", i);
    }

    // 6. Send 0x6200, then 0x620y
    exchange(0x6200.into());
    exchange(ExchangeUnit::new(0x62, multiboot_args.client_bit));
    gba::info!("Concluded cartridge data transfer");

    // 7. Send 0x63pp, where pp is palette data, until clients respond with
    //    0x73cc, where cc is a random byte. Store these bytes in client_data
    let mut palette_response: Option<[ExchangeUnit; 3]> = None;
    'palette_send_loop:
    for _i in 0..15 {
        let response = exchange(
            ExchangeUnit::new(0x63, palette_data)
        );
        for j in 0..response.len() {
            let unit = response[j];
            if unit.command != 0xff && unit.command != 0x73 {
                continue 'palette_send_loop;
            }
        }
        palette_response = Some(response);
        break;
    }
    if palette_response.is_none() {
        gba::error!("Could not get palette responses");
        return Err(0b0_01111_01111_11111);
    }
    for i in 0..palette_response.unwrap().len() {
        let client_data =  (palette_response.unwrap())[i].data;
        gba::info!("Got client data {} from client {}", client_data, i);
        multiboot_args.client_data[i] = client_data;
    }
    gba::info!("Sent palette data, got client data");

    // 8. Calculate handshake_data as 0x11 + sum of client_data bytes. Store it,
    //    and send 0x64HH
    
    multiboot_args.handshake_data = (0x11 as u16
        + multiboot_args.client_data[0] as u16
        + multiboot_args.client_data[1] as u16
        + multiboot_args.client_data[2] as u16) as u8;
    gba::info!("Calculated handshake as {}", multiboot_args.handshake_data);
    exchange(ExchangeUnit::new(0x64, multiboot_args.handshake_data));
    gba::info!("Sent handshake");

    // 9. Call SWI 0x25. If it succeeds, multiboot begins
    unsafe {
        match Multiboot(&multiboot_args, MultibootTransferMode::MultiPlay) {
            Ok(()) => {
                gba::info!("Multiboot successful");
                Ok(())
            },
            Err(()) => {
                gba::error!("Multiboot syscall failed");
                Err(0b0_11111_11111_11111)
            }
        }
    }
}