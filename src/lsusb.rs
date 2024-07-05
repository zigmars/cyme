//! Originally based on [libusb list_devices.rs example](https://github.com/dcuddeback/libusb-rs/blob/master/examples/list_devices.rs), attempts to mimic lsusb output and provide cross-platform [`crate::system_profiler::SPUSBDataType`] getter
//! Printing functions for lsusb style output of USB data
//!
//! The [lsusb source code](https://github.com/gregkh/usbutils/blob/master/lsusb.c) was used as a reference for a lot of the styling and content of the display module
//!
//! TODO:
//! - [ ] Implement do_otg: https://github.com/gregkh/usbutils/blob/master/lsusb.c#L3036
//! - [ ] Implement do_hub: https://github.com/gregkh/usbutils/blob/master/lsusb.c#L2805
//! - [ ] Implement do_debug: https://github.com/gregkh/usbutils/blob/master/lsusb.c#L2984
//! - [ ] Implement dump_bos_descriptor: https://github.com/gregkh/usbutils/blob/master/lsusb.c#L3437
//! - [ ] Convert the 'in dump' descriptor decoding into concrete structs in [`crate::usb::descriptors`] and use that for printing - like the [`crate::usb::descriptors::audio`] module
use crate::display::PrintSettings;
use crate::error::{Error, ErrorKind};
use crate::system_profiler;

use crate::usb::descriptors::audio;
use crate::usb::descriptors::video;
use crate::usb::descriptors::*;
use crate::usb::*;

mod audio_dumps;
pub mod names;
mod video_dumps;

use audio_dumps::*;
use video_dumps::*;

const TREE_LSUSB_BUS: &str = "/:  ";
const TREE_LSUSB_DEVICE: &str = "|__ ";
const TREE_LSUSB_SPACE: &str = "    ";

const LSUSB_DUMP_WIDTH: usize = 24;
const LSUSB_DUMP_INDENT_BASE: usize = 2;

/// Dump an array of value like lsusb
fn dump_array<T: std::fmt::Display>(array: &[T], field_name: &str, indent: usize, width: usize) {
    for (i, b) in array.iter().enumerate() {
        dump_value(b, &format!("{}({:2})", field_name, i), indent, width);
    }
}

/// Dump a bitmap value mapping as hex like lsusb
fn dump_bitmap_array<T: std::fmt::LowerHex + Into<u64> + Copy>(
    array: &[T],
    field_name: &str,
    indent: usize,
    width: usize,
) {
    for (i, b) in array.iter().enumerate() {
        dump_hex(*b, &format!("{}({:2})", field_name, i), indent, width);
    }
}

/// Dump just indented string
fn dump_string(field_name: &str, indent: usize) {
    println!("{:indent$}{}", "", field_name);
}

/// Dump a single value like lsusb
fn dump_value<T: std::fmt::Display>(value: T, field_name: &str, indent: usize, width: usize) {
    let value = value.to_string();
    let spaces = " ".repeat(
        (width - value.len())
            .saturating_sub(field_name.len())
            .max(1),
    );
    println!("{:indent$}{}{}{}", "", field_name, spaces, value,);
}

/// Dump a single hex value like lsusb
fn dump_hex<T: std::fmt::LowerHex + Into<u64>>(
    value: T,
    field_name: &str,
    indent: usize,
    width: usize,
) {
    let value_as_u64: u64 = value.into();
    let hex_value = format!(
        "0x{:0width$x}",
        value_as_u64,
        width = (std::mem::size_of::<T>() * 2)
    );
    dump_value(hex_value, field_name, indent, width);
}

/// Lookup the name of the value from passed function and dump it
fn dump_name<T: std::fmt::Display>(
    value: T,
    names_f: fn(T) -> Option<String>,
    field_name: &str,
    indent: usize,
    width: usize,
) {
    let value_string = value.to_string();
    let spaces = " ".repeat(
        (width - value_string.len())
            .saturating_sub(field_name.len())
            .max(1),
    );
    let dump = format!("{:indent$}{}{}{}", "", field_name, spaces, value_string,);
    if let Some(name) = names_f(value) {
        println!("{} {}", dump, name);
    }
}

/// Dumps the value and the string representation of the value to the right of width
fn dump_value_string<T: std::fmt::Display, S: std::fmt::Display>(
    value: T,
    field_name: &str,
    value_string: S,
    indent: usize,
    width: usize,
) {
    let value = value.to_string();
    let spaces = " ".repeat(
        (width - value.len())
            .saturating_sub(field_name.len())
            .max(1),
    );
    println!(
        "{:indent$}{}{}{} {}",
        "", field_name, spaces, value, value_string,
    );
}

/// Dumps a string starting at value position, right aligned
fn dump_string_right<T: std::fmt::Display>(guid: T, field_name: &str, indent: usize, width: usize) {
    // -1 to account for space
    let spaces = " ".repeat((width - 1).saturating_sub(field_name.len()).max(1));
    println!("{:indent$}{}{}{}", "", field_name, spaces, guid);
}

/// Dumps GUID enclosed in braces like lsusb
fn dump_guid(guid: &String, field_name: &str, indent: usize, width: usize) {
    dump_string_right(format!("{{{:}}}", guid), field_name, indent, width);
}

/// Dumps junk descriptor bytes as hex like lsusb
fn dump_junk(extra: &[u8], indent: usize, reported_len: usize, expected_len: usize) {
    if reported_len > expected_len && extra.len() >= reported_len {
        println!(
            "{:^indent$}junk at descriptor end: {}",
            "",
            extra[expected_len..reported_len]
                .iter()
                .map(|b| format!("{:02x}", b))
                .collect::<Vec<String>>()
                .join(" ")
        )
    }
}

/// Dumps unknown descriptor bytes as hex like lsusb
fn dump_unrecognised(extra: &[u8], indent: usize) {
    println!(
        "{:^indent$}** UNRECOGNIZED: {}",
        "",
        extra
            .iter()
            .map(|b| format!("{:02x}", b))
            .collect::<Vec<String>>()
            .join(" ")
    )
}

/// Dumps strings matching the bits set in `bitmap` using `strings_f` function from LSB to MSB
fn dump_bitmap_strings<T>(bitmap: T, strings_f: fn(usize) -> Option<&'static str>, indent: usize)
where
    T: std::fmt::Display + std::fmt::LowerHex + Copy + Into<u64>,
{
    let bitmap_u64: u64 = bitmap.into();
    let num_bits = std::mem::size_of::<T>() * 8;
    for index in 0..num_bits {
        if (bitmap_u64 >> index) & 0x1 != 0 {
            if let Some(string) = strings_f(index) {
                println!("{:indent$}{}", "", string);
            }
        }
    }
}

/// Dumps strings matching the bits set in `bitmap` using `strings_f` function from MSB to LSB
fn dump_bitmap_strings_invert<T>(
    bitmap: T,
    strings_f: fn(usize) -> Option<&'static str>,
    indent: usize,
) where
    T: std::fmt::Display + std::fmt::LowerHex + Copy + Into<u64>,
{
    let bitmap_u64: u64 = bitmap.into();
    let num_bits = std::mem::size_of::<T>() * 8;
    for index in (0..num_bits).rev() {
        if (bitmap_u64 >> index) & 0x1 != 0 {
            if let Some(string) = strings_f(index) {
                println!("{:indent$}{}", "", string);
            }
        }
    }
}

/// Dump a single value and the string representation of the value to the right of width
fn dump_bitmap_strings_inline<T, V>(
    value: V,
    bitmap: T,
    field_name: &str,
    strings_f: fn(usize) -> Option<&'static str>,
    indent: usize,
    width: usize,
) where
    T: std::fmt::Display + std::fmt::LowerHex + Copy + Into<u64>,
    V: std::fmt::Display,
{
    let value = value.to_string();
    let spaces = " ".repeat(
        (width - value.len())
            .saturating_sub(field_name.len())
            .max(1),
    );
    print!("{:indent$}{}{}{}", "", field_name, spaces, value,);
    let bitmap_u64: u64 = bitmap.into();
    let num_bits = std::mem::size_of::<T>() * 8;
    for index in 0..num_bits {
        if (bitmap_u64 >> index) & 0x1 != 0 {
            if let Some(string) = strings_f(index) {
                print!(" {}", string);
            }
        }
    }
    println!();
}

fn get_guid(buf: &[u8]) -> String {
    if buf.len() < 16 {
        return String::from("INVALID GUID");
    }

    format!("{{{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}}}", 
        buf[3], buf[2], buf[1], buf[0],
        buf[5], buf[4],
        buf[7], buf[6],
        buf[8], buf[9],
        buf[10], buf[11], buf[12], buf[13], buf[14], buf[15])
}

/// Print [`system_profiler::SPUSBDataType`] as a lsusb style tree with the two optional `verbosity` levels
pub fn print_tree(spusb: &system_profiler::SPUSBDataType, settings: &PrintSettings) {
    fn print_tree_devices(devices: &Vec<system_profiler::USBDevice>, settings: &PrintSettings) {
        let sorted = settings.sort_devices.sort_devices(devices);

        for device in sorted {
            if device.is_root_hub() {
                log::debug!("lsusb tree skipping root_hub {}", device);
                continue;
            }
            // the const len should get compiled to const...
            let indent = (device.get_depth() * TREE_LSUSB_DEVICE.len()) + TREE_LSUSB_SPACE.len();
            let device_tree_strings: Vec<(String, String, String)> = device.to_lsusb_tree_string();

            for strings in device_tree_strings {
                println!("{:>indent$}{}", TREE_LSUSB_DEVICE, strings.0);
                if settings.verbosity >= 1 {
                    println!("{:>indent$}{}", TREE_LSUSB_SPACE, strings.1);
                }
                if settings.verbosity >= 2 {
                    println!("{:>indent$}{}", TREE_LSUSB_SPACE, strings.2);
                }
            }
            // print all devices with this device - if hub for example
            device
                .devices
                .as_ref()
                .map_or((), |d| print_tree_devices(d, settings))
        }
    }

    for bus in &spusb.buses {
        let bus_tree_strings: Vec<(String, String, String)> = bus.to_lsusb_tree_string();
        for strings in bus_tree_strings {
            println!("{}{}", TREE_LSUSB_BUS, strings.0);
            if settings.verbosity >= 1 {
                println!("{}{}", TREE_LSUSB_SPACE, strings.1);
            }
            if settings.verbosity >= 2 {
                println!("{}{}", TREE_LSUSB_SPACE, strings.2);
            }
        }

        // followed by devices if there are some
        bus.devices
            .as_ref()
            .map_or((), |d| print_tree_devices(d, settings))
    }
}

/// Dump a single [`system_profiler::USBDevice`] matching `dev_path` verbosely
pub fn dump_one_device(
    devices: &Vec<&system_profiler::USBDevice>,
    dev_path: &String,
) -> Result<(), Error> {
    for device in devices {
        if &device.dev_path() == dev_path {
            // error if extra is none because we need it for vebose
            if device.extra.is_none() {
                return Err(Error::new(
                    ErrorKind::Opening,
                    &format!("Unable to open {}", dev_path),
                ));
            }

            print(&vec![device], true);
            return Ok(());
        }
    }

    Err(Error::new(
        ErrorKind::NotFound,
        &format!("Unable to find {}", dev_path),
    ))
}

/// Print USB devices in lsusb style flat dump
///
/// `verbose` flag enables verbose printing like lsusb (configs, interfaces and endpoints) - a huge dump!
pub fn print(devices: &Vec<&system_profiler::USBDevice>, verbose: bool) {
    if !verbose {
        for device in devices {
            println!("{}", device.to_lsusb_string());
        }
    } else {
        for device in devices {
            match device.extra.as_ref() {
                None => log::warn!(
                    "Skipping {} because it does not contain extra data required for verbose print",
                    device
                ),
                Some(device_extra) => {
                    println!(); // new lines separate in verbose lsusb
                    println!("{}", device.to_lsusb_string());
                    // print error regarding open if non-critcal during probe like lsusb --verbose
                    if device.profiler_error.is_some() {
                        eprintln!("Couldn't open device, some information will be missing");
                    }
                    dump_device(device);

                    let otg = false;
                    for config in &device_extra.configurations {
                        // TODO do_otg for config 0
                        dump_config(config, LSUSB_DUMP_INDENT_BASE);

                        for interface in &config.interfaces {
                            dump_interface(interface, LSUSB_DUMP_INDENT_BASE * 2);

                            for endpoint in &interface.endpoints {
                                dump_endpoint(endpoint, LSUSB_DUMP_INDENT_BASE * 3);
                            }
                        }
                    }

                    if let Some(bos) = &device_extra.binary_object_store {
                        // TODO dump_bos_descriptor
                        println!(
                            "{:indent$}BOS Descriptor: {:?}",
                            "",
                            bos,
                            indent = 0
                        );
                    }
                    if let Some(hub) = &device_extra.hub {
                        // TODO do_hub
                        println!(
                            "{:indent$}Hub Descriptor: {:?}",
                            "",
                            hub,
                            indent = 0
                        );
                    }
                    // lsusb do_dualspeed: dump_device_qualifier
                    if let Some(qualifier) = &device_extra.qualifier {
                        dump_device_qualifier(qualifier, 0);
                    }
                    if let Some(debug) = &device_extra.debug {
                        dump_debug(debug, 0);
                    }

                    dump_device_status(
                        device_extra.status.unwrap_or(0),
                        otg,
                        device.bcd_usb.map_or(false, |v| v.major() >= 3),
                        0,
                    );
                }
            }
        }
    }
}

/// Dump a [`system_profiler::USBDevice`] in style of lsusb --verbose
fn dump_device(device: &system_profiler::USBDevice) {
    let device_extra = device
        .extra
        .as_ref()
        .expect("Cannot print verbose without extra data");

    let (class_name, sub_class_name, protocol_name) =
        match (device.base_class_code(), device.sub_class, device.protocol) {
            (Some(bc), Some(scid), Some(pid)) => (
                names::class(bc),
                names::subclass(bc, scid),
                names::protocol(bc, scid, pid),
            ),
            (Some(bc), Some(scid), None) => (names::class(bc), names::subclass(bc, scid), None),
            (Some(bc), None, None) => (names::class(bc), None, None),
            (None, None, None) => (None, None, None),
            _ => unreachable!(),
        };

    println!("Device Descriptor:");
    // These are constants - length is 18 bytes for descriptor, type is 1
    dump_value(18, "bLength", 2, LSUSB_DUMP_WIDTH);
    dump_value(1, "bDescriptorType", 2, LSUSB_DUMP_WIDTH);
    dump_value(
        device
            .bcd_usb
            .as_ref()
            .map_or(String::new(), |v| v.to_string()),
        "bcdUSB",
        2,
        LSUSB_DUMP_WIDTH,
    );

    dump_value_string(
        device.base_class_code().unwrap_or(0),
        "bDeviceClass",
        class_name.unwrap_or(String::from("[unknown]")),
        2,
        LSUSB_DUMP_WIDTH,
    );

    dump_value_string(
        device.sub_class.unwrap_or(0),
        "bDeviceSubClass",
        sub_class_name.unwrap_or(String::from("[unknown]")),
        2,
        LSUSB_DUMP_WIDTH,
    );

    dump_value_string(
        device.protocol.unwrap_or(0),
        "bDeviceProtocol",
        protocol_name.unwrap_or_default(),
        2,
        LSUSB_DUMP_WIDTH,
    );

    dump_value(
        device_extra.max_packet_size,
        "bMaxPacketSize0",
        2,
        LSUSB_DUMP_WIDTH,
    );

    dump_value_string(
        format!("0x{:04x}", device.vendor_id.unwrap_or(0)),
        "idVendor",
        device_extra
            .vendor
            .as_ref()
            .unwrap_or(&String::from("[unknown]")),
        2,
        LSUSB_DUMP_WIDTH,
    );

    dump_value_string(
        format!("0x{:04x}", device.product_id.unwrap_or(0)),
        "idProduct",
        device_extra
            .product_name
            .as_ref()
            .unwrap_or(&String::from("[unknown]")),
        2,
        LSUSB_DUMP_WIDTH,
    );

    dump_value(
        device
            .bcd_device
            .as_ref()
            .map_or(String::new(), |v| v.to_string()),
        "bcdDevice",
        2,
        LSUSB_DUMP_WIDTH,
    );

    dump_value_string(
        device_extra.string_indexes.0,
        "iManufacturer",
        device
            .manufacturer
            .as_ref()
            .unwrap_or(&String::from("[unknown]")),
        2,
        LSUSB_DUMP_WIDTH,
    );

    dump_value_string(
        device_extra.string_indexes.1,
        "iProduct",
        &device.name,
        2,
        LSUSB_DUMP_WIDTH,
    );

    dump_value_string(
        device_extra.string_indexes.2,
        "iSerialNumber",
        device.serial_num.as_ref().unwrap_or(&String::new()),
        2,
        LSUSB_DUMP_WIDTH,
    );

    dump_value(
        device_extra.configurations.len(),
        "bNumConfigurations",
        2,
        LSUSB_DUMP_WIDTH,
    );
}

/// Dump a [`USBConfiguration`] in style of lsusb --verbose
fn dump_config(config: &USBConfiguration, indent: usize) {
    dump_string("Configuration Descriptor:", indent);
    dump_value(config.length, "bLength", indent + 2, LSUSB_DUMP_WIDTH);
    dump_value(2, "bDescriptorType", indent + 2, LSUSB_DUMP_WIDTH); // type 2 for configuration
    dump_value(
        config.total_length,
        "wTotalLength",
        indent + 2,
        LSUSB_DUMP_WIDTH,
    );
    dump_value(
        config.interfaces.len(),
        "bNumInterfaces",
        indent + 2,
        LSUSB_DUMP_WIDTH,
    );
    dump_value(
        config.number,
        "bConfigurationValue",
        indent + 2,
        LSUSB_DUMP_WIDTH,
    );
    dump_value_string(
        config.string_index,
        "iConfiguration",
        &config.name,
        indent + 2,
        LSUSB_DUMP_WIDTH,
    );
    dump_value(
        config.attributes_value(),
        "bmAttributes",
        indent + 2,
        LSUSB_DUMP_WIDTH,
    );
    // no attributes is bus powered
    if config.attributes.is_empty() {
        dump_string("(Bus Powered)", indent + 4);
    } else {
        if config.attributes.contains(&ConfigAttributes::SelfPowered) {
            dump_string("Self Powered", indent + 4);
        }
        if config.attributes.contains(&ConfigAttributes::RemoteWakeup) {
            dump_string("Remote Wakeup", indent + 4);
        }
    }
    dump_value(
        format!("{}{}", config.max_power.value, config.max_power.unit),
        "MaxPower",
        indent + 2,
        LSUSB_DUMP_WIDTH,
    );

    // dump extra descriptors
    if let Some(dt_vec) = &config.extra {
        for dt in dt_vec {
            match dt {
                DescriptorType::InterfaceAssociation(iad) => {
                    dump_interface_association(iad, indent + 2);
                }
                DescriptorType::Security(sec) => {
                    dump_security(sec, indent + 2);
                }
                DescriptorType::Encrypted(enc) => {
                    dump_encryption_type(enc, indent + 2);
                }
                DescriptorType::Unknown(junk) | DescriptorType::Junk(junk) => {
                    dump_unrecognised(junk, indent + 2);
                }
                _ => (),
            }
        }
    }
}

/// Dump a [`USBInterfaceAssociation`] in style of lsusb --verbose
fn dump_interface(interface: &USBInterface, indent: usize) {
    let interface_name = names::class(interface.class.into());
    let sub_class_name = names::subclass(interface.class.into(), interface.sub_class);
    let protocol_name = names::protocol(
        interface.class.into(),
        interface.sub_class,
        interface.protocol,
    );

    dump_string("Interface Descriptor:", indent);
    dump_value(interface.length, "bLength", indent + 2, LSUSB_DUMP_WIDTH);
    dump_value(4, "bDescriptorType", indent + 2, LSUSB_DUMP_WIDTH); // type 4 for interface
    dump_value(
        interface.number,
        "bInterfaceNumber",
        indent + 2,
        LSUSB_DUMP_WIDTH,
    );
    dump_value(
        interface.alt_setting,
        "bAlternateSetting",
        indent + 2,
        LSUSB_DUMP_WIDTH,
    );
    dump_value(
        interface.endpoints.len(),
        "bNumEndpoints",
        indent + 2,
        LSUSB_DUMP_WIDTH,
    );
    dump_value_string(
        u8::from(interface.class.to_owned()),
        "bInterfaceClass",
        interface_name.unwrap_or(String::from("[unknown]")),
        indent + 2,
        LSUSB_DUMP_WIDTH,
    );
    dump_value_string(
        interface.sub_class,
        "bInterfaceSubClass",
        sub_class_name.unwrap_or(String::from("[unknown]")),
        indent + 2,
        LSUSB_DUMP_WIDTH,
    );
    dump_value_string(
        interface.protocol,
        "bInterfaceProtocol",
        protocol_name.unwrap_or_default(),
        indent + 2,
        LSUSB_DUMP_WIDTH,
    );
    dump_value_string(
        interface.string_index,
        "iInterface",
        &interface.name,
        indent + 2,
        LSUSB_DUMP_WIDTH,
    );

    // dump extra descriptors
    if let Some(dt_vec) = &interface.extra {
        for dt in dt_vec {
            match dt {
                // Should only be Device or Interface as we mask out the rest
                DescriptorType::Device(cd) | DescriptorType::Interface(cd) => match cd {
                    ClassDescriptor::Hid(hidd) => dump_hid_device(hidd, indent + 2),
                    ClassDescriptor::Ccid(ccid) => dump_ccid_desc(ccid, indent + 2),
                    ClassDescriptor::Printer(pd) => dump_printer_desc(pd, indent + 2),
                    ClassDescriptor::Communication(cd) => dump_comm_descriptor(cd, indent + 2),
                    ClassDescriptor::Dfu(dfud) => dump_dfu_interface(dfud, indent + 2),
                    ClassDescriptor::Midi(md, _) => dump_midistreaming_interface(md, indent + 2),
                    ClassDescriptor::Audio(uacd, uacp) => match &uacd.subtype {
                        audio::UacType::Control(cs) => {
                            dump_audiocontrol_interface(uacd, cs, uacp, indent + 2)
                        }
                        audio::UacType::Streaming(ss) => {
                            dump_audiostreaming_interface(uacd, ss, uacp, indent + 2)
                        }
                        _ => (),
                    },
                    ClassDescriptor::Video(vcd, p) => match &vcd.subtype {
                        video::UvcType::Control(cs) => {
                            dump_videocontrol_interface(vcd, cs, *p, indent + 2)
                        }
                        video::UvcType::Streaming(ss) => {
                            dump_videostreaming_interface(vcd, ss, *p, indent + 2);
                        }
                    },
                    ClassDescriptor::Generic(cc, gd) => match cc {
                        Some((ClassCode::Audio, 3, _)) => {
                            if let Ok(md) = audio::MidiDescriptor::try_from(gd.to_owned()) {
                                dump_midistreaming_interface(&md, indent + 2);
                            }
                        }
                        Some((ClassCode::Audio, s, p)) => {
                            if let Ok(uacd) =
                                audio::UacDescriptor::try_from((gd.to_owned(), *s, *p))
                            {
                                let uacp = audio::UacProtocol::from(*p);
                                match &uacd.subtype {
                                    audio::UacType::Control(cs) => {
                                        dump_audiocontrol_interface(&uacd, cs, &uacp, indent + 2)
                                    }
                                    audio::UacType::Streaming(ss) => {
                                        dump_audiostreaming_interface(&uacd, ss, &uacp, indent + 2)
                                    }
                                    _ => (),
                                }
                            }
                        }
                        Some((ClassCode::Video, s, p)) => {
                            if let Ok(uvcd) =
                                video::UvcDescriptor::try_from((gd.to_owned(), *s, *p))
                            {
                                match &uvcd.subtype {
                                    video::UvcType::Control(cs) => {
                                        dump_videocontrol_interface(&uvcd, cs, *p, indent + 2);
                                    }
                                    video::UvcType::Streaming(ss) => {
                                        dump_videostreaming_interface(&uvcd, ss, *p, indent + 2);
                                    }
                                }
                            }
                        }
                        Some((ClassCode::ApplicationSpecificInterface, 1, _)) => {
                            if let Ok(dfud) = DfuDescriptor::try_from(gd.to_owned()) {
                                dump_dfu_interface(&dfud, indent + 2);
                            }
                        }
                        _ => {
                            let junk = Vec::from(cd.to_owned());
                            dump_unrecognised(&junk, indent + 2);
                        }
                    },
                },
                DescriptorType::Unknown(junk) | DescriptorType::Junk(junk) => {
                    dump_unrecognised(junk, 6);
                }
                _ => (),
            }
        }
    }
}

/// Dump a [`USBEndpoint`] in style of lsusb --verbose
fn dump_endpoint(endpoint: &USBEndpoint, indent: usize) {
    dump_string("Endpoint Descriptor:", indent);
    dump_value(endpoint.length, "bLength", indent + 2, LSUSB_DUMP_WIDTH);
    dump_value(5, "bDescriptorType", indent + 2, LSUSB_DUMP_WIDTH); // type 5 for endpoint
    dump_value_string(
        format!("0x{:02x}", endpoint.address.address),
        "bEndpointAddress",
        format!(
            "EP {} {}",
            endpoint.address.number,
            endpoint.address.direction.to_string().to_uppercase()
        ),
        indent + 2,
        LSUSB_DUMP_WIDTH,
    );
    // this is printed as int even though it's a bitmap
    dump_value(
        endpoint.attributes(),
        "bmAttributes",
        indent + 2,
        LSUSB_DUMP_WIDTH,
    );
    println!(
        "{:indent$}Transfer Type          {:?}",
        "",
        endpoint.transfer_type,
        indent = indent + 4
    );
    println!(
        "{:indent$}Sync Type              {:?}",
        "",
        endpoint.sync_type,
        indent = indent + 4
    );
    println!(
        "{:indent$}Usage Type             {:?}",
        "",
        endpoint.usage_type,
        indent = indent + 4
    );
    dump_value_string(
        format!("0x{:04x}", endpoint.max_packet_size),
        "wMaxPacketSize",
        format!("{} bytes", endpoint.max_packet_string()),
        indent + 2,
        LSUSB_DUMP_WIDTH,
    );
    dump_value(endpoint.interval, "bInterval", indent + 2, LSUSB_DUMP_WIDTH);

    // dump extra descriptors
    // kind of messy but it's out lsusb does it
    if let Some(dt_vec) = &endpoint.extra {
        for dt in dt_vec {
            match dt {
                DescriptorType::Endpoint(cd) => match cd {
                    ClassDescriptor::Audio(ad, _) => {
                        dump_audiostreaming_endpoint(ad, indent + 2);
                    }
                    // legacy as context should have been added to the descriptor
                    ClassDescriptor::Generic(cc, gd) => match cc {
                        Some((ClassCode::Audio, 2, p)) => {
                            if let Ok(uacd) = audio::UacDescriptor::try_from((gd.to_owned(), 2, *p))
                            {
                                dump_audiostreaming_endpoint(&uacd, indent + 2);
                            }
                        }
                        Some((ClassCode::Audio, 3, _)) => {
                            if let Ok(md) = audio::MidiEndpointDescriptor::try_from(gd.to_owned()) {
                                dump_midistreaming_endpoint(&md, indent + 2);
                            }
                        }
                        _ => (),
                    },
                    _ => (),
                },
                // Misplaced descriptors
                DescriptorType::Device(cd) => match cd {
                    ClassDescriptor::Ccid(ccid) => {
                        dump_ccid_desc(ccid, indent);
                    }
                    _ => {
                        println!(
                            "{:indent$}DEVICE CLASS: {}",
                            "",
                            Vec::<u8>::from(cd.to_owned())
                                .iter()
                                .map(|b| format!("{:02x}", b))
                                .collect::<Vec<String>>()
                                .join(" "),
                            indent = indent + 2
                        );
                    }
                },
                DescriptorType::Interface(cd) => match cd {
                    ClassDescriptor::Generic(cc, gd) => match cc {
                        Some((ClassCode::CDCData, _, _))
                        | Some((ClassCode::CDCCommunications, _, _)) => {
                            if let Ok(cd) = gd.to_owned().try_into() {
                                dump_comm_descriptor(&cd, indent)
                            }
                        }
                        Some((ClassCode::MassStorage, _, _)) => {
                            dump_pipe_desc(gd, indent + 2);
                        }
                        _ => {
                            println!(
                                "{:indent$}INTERFACE CLASS: {}",
                                "",
                                Vec::<u8>::from(cd.to_owned())
                                    .iter()
                                    .map(|b| format!("{:02x}", b))
                                    .collect::<Vec<String>>()
                                    .join(" "),
                                indent = indent + 2
                            );
                        }
                    },
                    ClassDescriptor::Communication(cd) => dump_comm_descriptor(cd, 6),
                    _ => {
                        println!(
                            "{:indent$}INTERFACE CLASS: {}",
                            "",
                            Vec::<u8>::from(cd.to_owned())
                                .iter()
                                .map(|b| format!("{:02x}", b))
                                .collect::<Vec<String>>()
                                .join(" "),
                            indent = indent + 2
                        );
                    }
                },
                DescriptorType::InterfaceAssociation(iad) => {
                    dump_interface_association(iad, indent + 2);
                }
                DescriptorType::SsEndpointCompanion(ss) => {
                    println!(
                        "{:indent$}bMaxBurst {:>14}",
                        "",
                        ss.max_burst,
                        indent = indent + 2
                    );
                    match endpoint.transfer_type {
                        TransferType::Bulk => {
                            if ss.attributes & 0x1f != 0 {
                                println!(
                                    "{:indent$}MaxStreams {:>13}",
                                    "",
                                    1 << ss.attributes,
                                    indent = indent + 2
                                );
                            }
                        }
                        TransferType::Isochronous => {
                            if ss.attributes & 0x03 != 0 {
                                println!(
                                    "{:indent$}Mult {:>19}",
                                    "",
                                    ss.attributes & 0x3,
                                    indent = indent + 2
                                );
                            }
                        }
                        _ => (),
                    }
                }
                DescriptorType::Unknown(junk) | DescriptorType::Junk(junk) => {
                    dump_unrecognised(junk, indent + 2);
                }
                _ => (),
            }
        }
    }
}

fn dump_ccid_desc(ccid: &CcidDescriptor, indent: usize) {
    dump_string("ChipCard Interface Descriptor:", indent);
    dump_value(ccid.length, "bLength", indent + 2, LSUSB_DUMP_WIDTH);
    dump_value(
        ccid.descriptor_type,
        "bDescriptorType",
        indent + 2,
        LSUSB_DUMP_WIDTH,
    );
    if ccid.version.major() != 1 || ccid.version.minor() != 0 {
        dump_value_string(
            ccid.version,
            "bcdCCID",
            "(Warning: Only accurate for version 1.0)",
            indent + 2,
            LSUSB_DUMP_WIDTH,
        );
    } else {
        dump_value(ccid.version, "bcdCCID", indent + 2, LSUSB_DUMP_WIDTH);
    }

    dump_value(
        ccid.max_slot_index,
        "bMaxSlotIndex",
        indent + 2,
        LSUSB_DUMP_WIDTH,
    );
    dump_bitmap_strings_inline(
        ccid.voltage_support,
        ccid.voltage_support,
        "bVoltageSupport",
        |index| match index {
            0 => Some("5.0V"),
            1 => Some("3.0V"),
            2 => Some("1.8V"),
            _ => None,
        },
        indent + 2,
        LSUSB_DUMP_WIDTH,
    );

    dump_bitmap_strings_inline(
        ccid.protocols,
        ccid.protocols,
        "dwProtocols",
        |index| match index {
            0 => Some("T=0"),
            1 => Some("T=1"),
            _ => Some("(Invalid values detected)"),
        },
        indent + 2,
        LSUSB_DUMP_WIDTH,
    );

    dump_value(
        ccid.default_clock,
        "dwDefaultClock",
        indent + 2,
        LSUSB_DUMP_WIDTH,
    );
    dump_value(
        ccid.max_clock,
        "dwMaxiumumClock",
        indent + 2,
        LSUSB_DUMP_WIDTH,
    );
    dump_value(
        ccid.num_clock_supported,
        "bNumClockSupported",
        indent + 2,
        LSUSB_DUMP_WIDTH,
    );
    dump_value_string(
        ccid.data_rate,
        "dwDataRate",
        "bps",
        indent + 2,
        LSUSB_DUMP_WIDTH,
    );
    dump_value_string(
        ccid.max_data_rate,
        "dwMaxDataRate",
        "bps",
        indent + 2,
        LSUSB_DUMP_WIDTH,
    );
    dump_value(
        ccid.num_data_rates_supp,
        "bNumDataRatesSupp.",
        indent + 2,
        LSUSB_DUMP_WIDTH,
    );
    dump_value(ccid.max_ifsd, "dwMaxIFSD", indent + 2, LSUSB_DUMP_WIDTH);
    dump_bitmap_strings_inline(
        format!("{:08X}", ccid.sync_protocols),
        ccid.sync_protocols,
        "dwSyncProtocols",
        |index| match index {
            0 => Some("2-wire"),
            1 => Some("3-wire"),
            2 => Some("I2C"),
            _ => None,
        },
        indent + 2,
        LSUSB_DUMP_WIDTH,
    );

    dump_bitmap_strings_inline(
        format!("{:08X}", ccid.mechanical),
        ccid.mechanical,
        "dwMechanical",
        |index| match index {
            0 => Some("accept"),
            1 => Some("eject"),
            2 => Some("capture"),
            3 => Some("lock"),
            _ => None,
        },
        indent + 2,
        LSUSB_DUMP_WIDTH,
    );

    dump_value(
        format!("{:08X}", ccid.features),
        "dwFeatures",
        indent + 2,
        LSUSB_DUMP_WIDTH,
    );
    dump_bitmap_strings(
        ccid.features,
        |index| match index {
            0 => Some("Auto configuration based on ATR"),
            1 => Some("Auto activation on insert"),
            2 => Some("Auto voltage selection"),
            3 => Some("Auto clock change"),
            4 => Some("Auto baud rate change"),
            5 => Some("Auto parameter negotiation made by CCID"),
            6 => Some("Auto PPS made by CCID"),
            7 => Some("CCID can set ICC in clock stop mode"),
            8 => Some("NAD value other than 0x00 accepted"),
            9 => Some("Auto IFSD exchange"),
            16 => Some("TPDU level exchange"),
            17 => Some("Short APDU level exchange"),
            18 => Some("Short and extended APDU level exchange"),
            _ => None,
        },
        indent + 4,
    );
    if (ccid.features & (0x0040 | 0x0080)) != 0 {
        println!(
            "{:indent$}WARNING: conflicting negotiation features",
            "",
            indent = indent + 2
        );
    }
    if ccid.features & 0x00070000 != 0 {
        println!(
            "{:indent$}WARNING: conflicting exchange levels",
            "",
            indent = indent + 2
        );
    }

    dump_value(
        ccid.max_ccid_msg_len,
        "dwMaxCCIDMsgLen",
        indent + 2,
        LSUSB_DUMP_WIDTH,
    );
    if ccid.class_get_response == 0xff {
        dump_value("echo", "bClassGetResponse", indent + 2, LSUSB_DUMP_WIDTH);
    } else {
        dump_value(
            format!("{:02X}", ccid.class_get_response),
            "bClassGetResponse",
            indent + 2,
            LSUSB_DUMP_WIDTH,
        );
    }

    if ccid.class_envelope == 0xff {
        dump_value("echo", "bClassEnvelope", indent + 2, LSUSB_DUMP_WIDTH);
    } else {
        dump_value(
            format!("{:02X}", ccid.class_envelope),
            "bClassEnvelope",
            indent + 2,
            LSUSB_DUMP_WIDTH,
        );
    }

    if ccid.lcd_layout == (0, 0) {
        dump_value("none", "wlcdLayout", indent + 2, LSUSB_DUMP_WIDTH);
    } else {
        dump_value_string(
            ccid.lcd_layout.0,
            "wlcdLayout",
            format!(" cols {} lines", ccid.lcd_layout.1),
            indent + 2,
            LSUSB_DUMP_WIDTH,
        );
    }

    dump_bitmap_strings_inline(
        ccid.pin_support,
        ccid.pin_support,
        "bPINSupport",
        |index| match index {
            0 => Some("verification"),
            1 => Some("modification"),
            _ => None,
        },
        indent + 2,
        LSUSB_DUMP_WIDTH,
    );

    dump_value(
        ccid.max_ccid_busy_slots,
        "bMaxCCIDBusySlots",
        indent + 2,
        LSUSB_DUMP_WIDTH,
    );
}

fn dump_printer_desc(pd: &PrinterDescriptor, indent: usize) {
    dump_string("Printer Interface Descriptor:", indent);
    dump_value(pd.length, "bLength", indent + 2, LSUSB_DUMP_WIDTH);
    dump_value(
        pd.descriptor_type,
        "bDescriptorType",
        indent + 2,
        LSUSB_DUMP_WIDTH,
    );
    dump_value(
        pd.release_number,
        "bcdReleaseNumber",
        indent + 2,
        LSUSB_DUMP_WIDTH,
    );
    dump_value(
        pd.descriptors.len(),
        "bcdNumDescriptors",
        indent + 2,
        LSUSB_DUMP_WIDTH,
    );

    for desc in &pd.descriptors {
        // basic capabilities
        if desc.descriptor_type == 0x00 {
            dump_value(
                desc.versions_supported,
                "iIPPVersionsSupported",
                indent + 2,
                LSUSB_DUMP_WIDTH,
            );
            dump_value_string(
                desc.uuid_string_index,
                "iIPPPrinterUUID",
                desc.uuid_string.as_ref().unwrap_or(&String::new()),
                indent + 2,
                LSUSB_DUMP_WIDTH,
            );
            print!(
                "{:indent$}wBasicCapabilities   0x{:04x} ",
                "",
                desc.capabilities,
                indent = indent + 2
            );

            // capabilities
            if desc.capabilities & 0x0001 != 0 {
                print!(" Print");
            }
            if desc.capabilities & 0x0002 != 0 {
                print!(" Scan");
            }
            if desc.capabilities & 0x0004 != 0 {
                print!(" Fax");
            }
            if desc.capabilities & 0x0008 != 0 {
                print!(" Other");
            }
            if desc.capabilities & 0x0010 != 0 {
                print!(" HTTP-over-USB");
            }
            if (desc.capabilities & 0x0060) != 0 {
                print!(" No-Auth");
            } else if (desc.capabilities & 0x0060) != 0x20 {
                print!(" Username-Auth");
            } else if (desc.capabilities & 0x0060) != 0x40 {
                print!(" Reserved-Auth");
            } else if (desc.capabilities & 0x0060) != 0x60 {
                print!(" Negotiable-Auth");
            }
            println!();
        // vendor specific
        } else {
            dump_value_string(
                desc.descriptor_type,
                "UnknownCapabilities",
                desc.length,
                indent + 2,
                LSUSB_DUMP_WIDTH,
            );
        }
    }
}

fn dump_bad_comm(cd: &CommunicationDescriptor, indent: usize) {
    let data = Into::<Vec<u8>>::into(cd.to_owned());
    println!(
        "{:^indent$}INVALID CDC ({:#}): {}",
        "",
        cd.communication_type,
        data.iter()
            .map(|b| format!("{:02x}", b))
            .collect::<Vec<String>>()
            .join(" ")
    );
}

fn dump_comm_descriptor(cd: &CommunicationDescriptor, indent: usize) {
    match cd.communication_type {
        CdcType::Header => {
            if cd.data.len() >= 2 {
                dump_string("CDC Header:", indent);
                dump_value(
                    format!("{:x}.{:02x}", cd.data[1], cd.data[0]),
                    "bcdCDC",
                    indent + 2,
                    LSUSB_DUMP_WIDTH,
                );
            } else {
                dump_bad_comm(cd, indent);
            }
        }
        CdcType::CallManagement => {
            if cd.data.len() >= 2 {
                dump_string("CDC Call Management:", indent);
                dump_hex(cd.data[0], "bmCapabilities", indent + 2, LSUSB_DUMP_WIDTH);
                dump_bitmap_strings(
                    cd.data[0],
                    |b| match b {
                        0 => Some("call management"),
                        1 => Some("dataInterface"),
                        _ => None,
                    },
                    indent + 4,
                );
            } else {
                dump_bad_comm(cd, indent);
            }
        }
        CdcType::AbstractControlManagement => {
            if !cd.data.is_empty() {
                dump_string("CDC ACM:", indent);
                dump_hex(cd.data[0], "bmCapabilities", indent + 2, LSUSB_DUMP_WIDTH);
                dump_bitmap_strings_invert(
                    cd.data[0],
                    |b| match b {
                        0 => Some("get/set/clear comm features"),
                        1 => Some("line coding and serial state"),
                        2 => Some("sends break"),
                        3 => Some("connection notifications"),
                        _ => None,
                    },
                    indent + 4,
                );
            } else {
                dump_bad_comm(cd, indent);
            }
        }
        CdcType::Union => {
            if cd.data.len() >= 2 {
                dump_string("CDC Union:", indent);
                dump_value(cd.data[0], "bMasterInterface", indent + 2, LSUSB_DUMP_WIDTH);
                println!(
                    "{:indent$}bSlaveInterface      {}",
                    "",
                    cd.data[1..]
                        .iter()
                        .map(|b| format!("{:3}", b))
                        .collect::<Vec<String>>()
                        .join(" "),
                    indent = indent + 2
                );
            } else {
                dump_bad_comm(cd, indent);
            }
        }
        CdcType::CountrySelection => {
            if cd.data.len() >= 3 || (cd.length & 0x01) != 0 {
                dump_string("Country Selection:", indent);
                dump_value_string(
                    cd.string_index.unwrap_or_default(),
                    "iCountryCodeRelDate",
                    cd.string.as_ref().unwrap_or(&String::from("(?)")),
                    indent + 2,
                    LSUSB_DUMP_WIDTH,
                );
                cd.data.chunks(2).for_each(|d| {
                    dump_value(
                        format!("{:02x}{:02x}", d[1], d[0]),
                        "wCountryCode",
                        indent + 2,
                        LSUSB_DUMP_WIDTH,
                    );
                });
            } else {
                dump_bad_comm(cd, indent);
            }
        }
        CdcType::TelephoneOperationalModes => {
            if !cd.data.is_empty() {
                dump_string("CDC Telephone operations:", indent);
                dump_hex(cd.data[0], "bmCapabilities", indent + 2, LSUSB_DUMP_WIDTH);
                dump_bitmap_strings_invert(
                    cd.data[0],
                    |b| match b {
                        0 => Some("simple mode"),
                        1 => Some("standalone mode"),
                        2 => Some("computer centric mode"),
                        _ => None,
                    },
                    indent + 4,
                );
            } else {
                dump_bad_comm(cd, indent);
            }
        }
        CdcType::NetworkChannel => {
            if cd.data.len() >= 4 {
                dump_string("Network Channel Terminal:", indent);
                dump_value(cd.data[0], "bEntityId", indent + 2, LSUSB_DUMP_WIDTH);
                dump_value_string(
                    cd.string_index.unwrap_or_default(),
                    "iName",
                    cd.string.as_ref().unwrap_or(&String::from("(?)")),
                    indent + 2,
                    LSUSB_DUMP_WIDTH,
                );
                dump_value(cd.data[2], "bChannelIndex", indent + 2, LSUSB_DUMP_WIDTH);
                dump_value(
                    cd.data[3],
                    "bPhysicalInterface",
                    indent + 2,
                    LSUSB_DUMP_WIDTH,
                );
            } else {
                dump_bad_comm(cd, indent);
            }
        }
        CdcType::EthernetNetworking => {
            if cd.data.len() >= 10 {
                dump_string("CDC Ethernet:", indent);
                dump_value_string(
                    cd.string_index.unwrap_or_default(),
                    "iMacAddress",
                    cd.string.as_ref().unwrap_or(&String::from("(?)")),
                    indent + 2,
                    LSUSB_DUMP_WIDTH,
                );
                dump_hex(
                    u32::from_le_bytes([cd.data[1], cd.data[2], cd.data[3], cd.data[4]]),
                    "bmEthernetStatistics",
                    indent + 2,
                    LSUSB_DUMP_WIDTH,
                );
                dump_value(
                    u16::from_le_bytes([cd.data[5], cd.data[6]]),
                    "wMaxSegmentSize",
                    indent + 2,
                    LSUSB_DUMP_WIDTH,
                );
                dump_hex(
                    u16::from_le_bytes([cd.data[7], cd.data[8]]),
                    "wNumberMCFilters",
                    indent + 2,
                    LSUSB_DUMP_WIDTH,
                );
                dump_hex(
                    cd.data[9],
                    "bNumberPowerFilters",
                    indent + 2,
                    LSUSB_DUMP_WIDTH,
                );
            } else {
                dump_bad_comm(cd, indent);
            }
        }
        CdcType::WirelessHandsetControlModel => {
            if cd.data.len() >= 2 {
                dump_string("CDC WHCM:", indent);
                dump_value(
                    format!("{:x}.{:02x}", cd.data[1], cd.data[0]),
                    "bcdVersion",
                    indent + 2,
                    LSUSB_DUMP_WIDTH,
                );
            } else {
                dump_bad_comm(cd, indent);
            }
        }
        CdcType::MobileDirectLineModelFunctional => {
            if cd.data.len() >= 18 {
                dump_string("CDC MDLM:", indent);
                dump_value(
                    format!("{:x}.{:02x}", cd.data[1], cd.data[0]),
                    "bcdVersion",
                    indent + 2,
                    LSUSB_DUMP_WIDTH,
                );
                dump_value_string(
                    "",
                    "bGUID",
                    get_guid(&cd.data[2..18]),
                    indent + 2,
                    LSUSB_DUMP_WIDTH - 2,
                );
            } else {
                dump_bad_comm(cd, indent);
            }
        }
        CdcType::MobileDirectLineModelDetail => {
            if cd.data.len() >= 2 {
                dump_string("CDC MDLM detail:", indent);
                dump_value(
                    format!("{:02x}", cd.data[0]),
                    "bGuidDescriptorType",
                    indent + 2,
                    LSUSB_DUMP_WIDTH,
                );
                println!(
                    "{:indent$}bDetailData          {}",
                    "",
                    cd.data
                        .iter()
                        .map(|b| format!("{:02x}", b))
                        .collect::<Vec<String>>()
                        .join(" "),
                    indent = indent + 2
                );
            } else {
                dump_bad_comm(cd, indent);
            }
        }
        CdcType::DeviceManagement => {
            if cd.data.len() >= 4 {
                dump_string("CDC MDLM:", indent);
                dump_value(
                    format!("{:x}.{:02x}", cd.data[1], cd.data[0]),
                    "bcdVersion",
                    indent + 2,
                    LSUSB_DUMP_WIDTH,
                );
                dump_value(
                    u16::from_le_bytes([cd.data[2], cd.data[3]]),
                    "wMaxCommand",
                    indent + 2,
                    LSUSB_DUMP_WIDTH,
                );
            } else {
                dump_bad_comm(cd, indent);
            }
        }
        CdcType::Obex => {
            if cd.data.len() >= 2 {
                dump_string("CDC OBEX:", indent);
                dump_value(
                    format!("{:x}.{:02x}", cd.data[1], cd.data[0]),
                    "bcdVersion",
                    indent + 2,
                    LSUSB_DUMP_WIDTH,
                );
            } else {
                dump_bad_comm(cd, indent);
            }
        }
        CdcType::CommandSet => {
            if cd.data.len() >= 19 {
                dump_string("CDC Command Set:", indent);
                dump_value(
                    format!("{:x}.{:02x}", cd.data[1], cd.data[0]),
                    "bcdVersion",
                    indent + 2,
                    LSUSB_DUMP_WIDTH,
                );
                dump_value_string(
                    cd.string_index.unwrap_or_default(),
                    "iCommandSet",
                    cd.string.as_ref().unwrap_or(&String::from("(?)")),
                    indent + 2,
                    LSUSB_DUMP_WIDTH,
                );
                dump_value_string(
                    "",
                    "bGUID",
                    get_guid(&cd.data[3..19]),
                    indent + 2,
                    LSUSB_DUMP_WIDTH - 2,
                );
            } else {
                dump_bad_comm(cd, indent);
            }
        }
        CdcType::Ncm => {
            if cd.data.len() >= 6 - 3 {
                dump_string("CDC NCM:", indent);
                dump_value(
                    format!("{:x}.{:02x}", cd.data[1], cd.data[0]),
                    "bcdNcmVersion",
                    indent + 2,
                    LSUSB_DUMP_WIDTH,
                );
                dump_hex(
                    cd.data[2],
                    "bmNetworkCapabilities",
                    indent + 2,
                    LSUSB_DUMP_WIDTH,
                );
                dump_bitmap_strings_invert(
                    cd.data[2],
                    |b| match b {
                        0 => Some("packet filter"),
                        1 => Some("net address"),
                        2 => Some("encapsulated commands"),
                        3 => Some("max cd.datagram size"),
                        4 => Some("crc mode"),
                        5 => Some("8-byte ntb input size"),
                        _ => None,
                    },
                    indent + 4,
                );
            } else {
                dump_bad_comm(cd, indent);
            }
        }
        CdcType::Mbim => {
            if cd.data.len() >= 9 {
                dump_string("CDC MBIM:", indent);
                dump_value(
                    format!("{:x}.{:02x}", cd.data[1], cd.data[0]),
                    "bcdMBIMVersion",
                    indent + 2,
                    LSUSB_DUMP_WIDTH,
                );
                dump_value(
                    u16::from_le_bytes([cd.data[2], cd.data[3]]),
                    "wMaxControlMessage",
                    indent + 2,
                    LSUSB_DUMP_WIDTH,
                );
                dump_value(cd.data[4], "bNumberFilters", indent + 2, LSUSB_DUMP_WIDTH);
                dump_value(cd.data[5], "bMaxFilterSize", indent + 2, LSUSB_DUMP_WIDTH);
                dump_value(
                    u16::from_le_bytes([cd.data[6], cd.data[7]]),
                    "wMaxSegmentSize",
                    indent + 2,
                    LSUSB_DUMP_WIDTH,
                );
                dump_hex(
                    cd.data[8],
                    "bmNetworkCapabilities",
                    indent + 2,
                    LSUSB_DUMP_WIDTH,
                );
                dump_bitmap_strings_invert(
                    cd.data[8],
                    |b| match b {
                        3 => Some("max cd.datagram size"),
                        5 => Some("8-byte ntb input size"),
                        _ => None,
                    },
                    indent + 4,
                );
            } else {
                dump_bad_comm(cd, indent);
            }
        }
        CdcType::MbimExtended => {
            if cd.data.len() >= 5 {
                dump_string("CDC MBIM Extended:", indent);
                dump_value(
                    format!("{:x}.{:02x}", cd.data[1], cd.data[0]),
                    "bcdMBIMExtendedVersion",
                    indent + 2,
                    LSUSB_DUMP_WIDTH,
                );
                dump_value(
                    cd.data[2],
                    "bMaxOutstandingCommandMessages",
                    indent + 2,
                    LSUSB_DUMP_WIDTH,
                );
                dump_value(
                    u16::from_le_bytes([cd.data[3], cd.data[4]]),
                    "wMTU",
                    indent + 2,
                    LSUSB_DUMP_WIDTH,
                );
            } else {
                dump_bad_comm(cd, indent);
            }
        }
        _ => {
            println!(
                "{:^indent$}UNRECOGNIZED CDC: {}",
                "",
                Vec::<u8>::from(cd.to_owned())
                    .iter()
                    .map(|b| format!("{:02x}", b))
                    .collect::<Vec<String>>()
                    .join(" "),
                indent = indent
            );
        }
    }
}

fn dump_dfu_interface(dfud: &DfuDescriptor, indent: usize) {
    // wider in lsusb but I prefer standard
    //const DFU_WIDTH: usize = 36;
    const DFU_WIDTH: usize = LSUSB_DUMP_WIDTH;

    dump_string("Device Firmware Upgrade Interface Descriptor:", indent);
    dump_value(dfud.length, "bLength", indent + 2, DFU_WIDTH);
    dump_value(
        dfud.descriptor_type,
        "bDescriptorType",
        indent + 2,
        DFU_WIDTH,
    );
    dump_value(dfud.attributes, "bmAttributes", indent + 2, DFU_WIDTH);

    if dfud.attributes & 0xf0 != 0 {
        println!("{:indent$}(unknown attributes!)", "", indent = indent + 4);
    }
    if dfud.attributes & 0x08 != 0 {
        println!("{:indent$}Will Detach", "", indent = indent + 4);
    } else {
        println!("{:indent$}Will Not Detach", "", indent = indent + 4);
    }
    if dfud.attributes & 0x04 != 0 {
        println!(
            "{:indent$}Manifestation Intolerant",
            "",
            indent = indent + 4
        );
    } else {
        println!("{:indent$}Manifestation Tolerant", "", indent = indent + 4);
    }
    if dfud.attributes & 0x02 != 0 {
        println!("{:indent$}Upload Supported", "", indent = indent + 4);
    } else {
        println!("{:indent$}Upload Unsupported", "", indent = indent + 4);
    }
    if dfud.attributes & 0x01 != 0 {
        println!("{:indent$}Download Supported", "", indent = indent + 4);
    } else {
        println!("{:indent$}Download Unsupported", "", indent = indent + 4);
    }

    dump_value_string(
        dfud.detach_timeout,
        "wDetachTimeout",
        "milliseconds",
        indent + 2,
        DFU_WIDTH,
    );
    dump_value_string(
        dfud.transfer_size,
        "wTransferSize",
        "bytes",
        indent + 2,
        DFU_WIDTH,
    );
    if let Some(bcd) = dfud.dfu_version.as_ref() {
        dump_value(bcd, "bcdDFUVersion", indent + 2, DFU_WIDTH);
    }
}

fn dump_pipe_desc(gd: &GenericDescriptor, indent: usize) {
    if gd.length == 4 && gd.descriptor_type == 0x24 {
        let subtype_string = match gd.descriptor_subtype {
            1 => "Command pipe",
            2 => "Status pipe",
            3 => "Data-in pipe",
            4 => "Data-out pipe",
            0 | 5..=0xdf | 0xf0..=0xff => "Reserved",
            0xe0..=0xef => "Vendor-specific",
        };

        println!(
            "{:indent$}{} (0x{:02x})",
            "",
            subtype_string,
            gd.descriptor_subtype,
            indent = indent
        );
    } else {
        println!(
            "{:indent$}INTERFACE CLASS: {}",
            "",
            Vec::<u8>::from(gd.to_owned())
                .iter()
                .map(|b| format!("{:02x}", b))
                .collect::<Vec<String>>()
                .join(" "),
            indent = indent
        );
    }
}

fn dump_security(sec: &SecurityDescriptor, indent: usize) {
    dump_string("Security Descriptor:", indent);
    dump_value(sec.length, "bLength", indent + 2, LSUSB_DUMP_WIDTH);
    dump_value(
        sec.descriptor_type,
        "bDescriptorType",
        indent + 2,
        LSUSB_DUMP_WIDTH,
    );
    dump_hex(
        sec.total_length,
        "wTotalLength",
        indent + 2,
        LSUSB_DUMP_WIDTH,
    );
    dump_value(
        sec.encryption_types,
        "bNumEncryptionTypes",
        indent + 2,
        LSUSB_DUMP_WIDTH,
    );
}

fn dump_encryption_type(enc: &EncryptionDescriptor, indent: usize) {
    let enct_string = match enc.encryption_type as u8 {
        0 => "UNSECURE",
        1 => "WIRED",
        2 => "CCM_1",
        3 => "RSA_1",
        _ => "RESERVED",
    };

    dump_string("Encryption Type:", indent);
    dump_value(enc.length, "bLength", indent + 2, LSUSB_DUMP_WIDTH);
    dump_value(
        enc.descriptor_type,
        "bDescriptorType",
        indent + 2,
        LSUSB_DUMP_WIDTH,
    );
    dump_value_string(
        enc.encryption_type as u8,
        "bEncryptionType",
        enct_string,
        indent + 2,
        LSUSB_DUMP_WIDTH,
    );
    dump_value(
        enc.encryption_value,
        "bEncryptionValue",
        indent + 2,
        LSUSB_DUMP_WIDTH,
    );
    dump_value(
        enc.auth_key_index,
        "bAuthKeyIndex",
        indent + 2,
        LSUSB_DUMP_WIDTH,
    );
}

fn dump_interface_association(iad: &InterfaceAssociationDescriptor, indent: usize) {
    dump_string("Interface Association:", indent);
    dump_value(iad.length, "bLength", indent + 2, LSUSB_DUMP_WIDTH);
    dump_value(
        iad.descriptor_type,
        "bDescriptorType",
        indent + 2,
        LSUSB_DUMP_WIDTH,
    );
    dump_value(
        iad.first_interface,
        "bFirstInterface",
        indent + 2,
        LSUSB_DUMP_WIDTH,
    );
    dump_value(
        iad.interface_count,
        "bInterfaceCount",
        indent + 2,
        LSUSB_DUMP_WIDTH,
    );
    dump_value_string(
        iad.function_class,
        "bFunctionClass",
        names::class(iad.function_class).unwrap_or_default(),
        indent + 2,
        LSUSB_DUMP_WIDTH,
    );
    dump_value_string(
        iad.function_sub_class,
        "bFunctionSubClass",
        names::subclass(iad.function_class, iad.function_sub_class).unwrap_or_default(),
        indent + 2,
        LSUSB_DUMP_WIDTH,
    );
    dump_value_string(
        iad.function_protocol,
        "bFunctionProtocol",
        names::protocol(
            iad.function_class,
            iad.function_sub_class,
            iad.function_protocol,
        )
        .unwrap_or_default(),
        indent + 2,
        LSUSB_DUMP_WIDTH,
    );
    dump_value_string(
        iad.function_string_index,
        "iFunction",
        iad.function_string.as_ref().unwrap_or(&String::new()),
        indent + 2,
        LSUSB_DUMP_WIDTH,
    );
}

fn dump_hid_device(hidd: &HidDescriptor, indent: usize) {
    dump_string("HID Descriptor:", indent);
    dump_value(hidd.length, "bLength", indent + 2, LSUSB_DUMP_WIDTH);
    dump_value(
        hidd.descriptor_type,
        "bDescriptorType",
        indent + 2,
        LSUSB_DUMP_WIDTH,
    );
    dump_value(hidd.bcd_hid, "bcdHID", indent + 2, LSUSB_DUMP_WIDTH);
    dump_value_string(
        hidd.country_code,
        "bCountryCode",
        names::countrycode(hidd.country_code).unwrap_or_default(),
        indent + 2,
        LSUSB_DUMP_WIDTH,
    );
    dump_value(
        hidd.descriptors.len(),
        "bNumDescriptors",
        indent + 2,
        LSUSB_DUMP_WIDTH,
    );
    for desc in &hidd.descriptors {
        dump_value_string(
            desc.descriptor_type,
            "bDescriptorType",
            names::hid(desc.descriptor_type).unwrap_or_default(),
            indent + 2,
            LSUSB_DUMP_WIDTH,
        );
        dump_value(
            desc.length,
            "wDescriptorLength",
            indent + 2,
            LSUSB_DUMP_WIDTH,
        );
    }

    for desc in &hidd.descriptors {
        // only print report descriptor
        if desc.descriptor_type != 0x22 {
            continue;
        }

        match desc.data.as_ref() {
            Some(d) => {
                dump_report_desc(d, indent + 2);
            }
            None => {
                dump_string("Report Descriptors:", indent + 2);
                dump_string("** UNAVAILABLE **", indent + 4);
            }
        }
    }
}

fn dump_device_qualifier(dqd: &DeviceQualifierDescriptor, indent: usize) {
    dump_string("Device Qualifier:", indent);
    dump_value(dqd.length, "bLength", indent + 2, LSUSB_DUMP_WIDTH);
    dump_value(
        dqd.descriptor_type,
        "bDescriptorType",
        indent + 2,
        LSUSB_DUMP_WIDTH,
    );
    dump_value(dqd.version, "bcdUSB", indent + 2, LSUSB_DUMP_WIDTH);
    let class: u8 = dqd.device_class as u8;
    dump_value_string(
        class,
        "bDeviceClass",
        dqd.device_class,
        indent + 2,
        LSUSB_DUMP_WIDTH,
    );
    dump_value_string(
        dqd.device_subclass,
        "bDeviceSubClass",
        names::subclass(class, dqd.device_subclass).unwrap_or(String::from("[unknown]")),
        indent + 2,
        LSUSB_DUMP_WIDTH,
    );
    dump_value_string(
        dqd.device_protocol,
        "bDeviceProtocol",
        names::protocol(class, dqd.device_subclass, dqd.device_protocol)
            .unwrap_or(String::from("[unknown]")),
        indent + 2,
        LSUSB_DUMP_WIDTH,
    );
    dump_value(
        dqd.max_packet_size,
        "bMaxPacketSize0",
        indent + 2,
        LSUSB_DUMP_WIDTH,
    );
    dump_value(
        dqd.num_configurations,
        "bNumConfigurations",
        indent + 2,
        LSUSB_DUMP_WIDTH,
    );
}

fn dump_debug(dd: &DebugDescriptor, indent: usize) {
    dump_string("Debug Descriptor:", indent);
    dump_value(dd.length, "bLength", indent + 2, LSUSB_DUMP_WIDTH);
    dump_value(
        dd.descriptor_type,
        "bDescriptorType",
        indent + 2,
        LSUSB_DUMP_WIDTH,
    );
    dump_hex(
        dd.debug_in_endpoint,
        "bDebugInEndpoint",
        indent + 2,
        LSUSB_DUMP_WIDTH,
    );
    dump_hex(
        dd.debug_out_endpoint,
        "bDebugOutEndpoint",
        indent + 2,
        LSUSB_DUMP_WIDTH,
    );
}

fn dump_device_status(status: u16, otg: bool, super_speed: bool, indent: usize) {
    dump_hex(status, "Device Status:", indent, LSUSB_DUMP_WIDTH);
    if status & 0x01 != 0 {
        println!("{:indent$}Self Powered", "", indent = indent + 2);
    } else {
        println!("{:indent$}(Bus Powered)", "", indent = indent + 2);
    }
    if status & 0x02 != 0 {
        println!("{:indent$}Remote Wakeup Enabled", "", indent = indent + 2);
    }
    if super_speed {
        if status & (1 << 2) != 0 {
            println!("{:indent$}U1 Enabled", "", indent = indent + 2);
        }
        if status & (1 << 3) != 0 {
            println!("{:indent$}U2 Enabled", "", indent = indent + 2);
        }
        if status & (1 << 4) != 0 {
            println!(
                "{:indent$}Latency Tolerance Messaging (LTM) Enabled",
                "",
                indent = indent + 2
            );
        }
    }
    if otg {
        if status & (1 << 3) != 0 {
            println!("{:indent$}HNP Enabled", "", indent = indent + 2);
        }
        if status & (1 << 4) != 0 {
            println!("{:indent$}HNP Capable", "", indent = indent + 2);
        }
        if status & (1 << 5) != 0 {
            println!("{:indent$}ALT port is HNP Capable", "", indent = indent + 2);
        }
    }
    if status & (1 << 6) != 0 {
        println!("{:indent$}Debug Mode", "", indent = indent + 2);
    }
}

/// Verbatum port of lsusb's dump_unit - not very Rust, don't judge!
fn dump_unit(mut data: u16, len: usize, indent: usize) {
    let systems = |t: u16| match t {
        0x01 => "SI Linear",
        0x02 => "SI Rotation",
        0x03 => "English Linear",
        0x04 => "English Rotation",
        _ => "None",
    };
    let units = |t: u16, i: usize| match (t, i) {
        (1, 1) => "Centimeter",
        (2, 1) => "Radians",
        (1, 2) | (2, 2) => "Gram",
        (1, 4) | (2, 4) => "Kelvin",
        (3, 1) => "Inch",
        (4, 1) => "Degrees",
        (1, i) | (2, i) | (3, i) | (4, i) => match i {
            0x02 => "Slug",
            0x03 => "Seconds",
            0x04 => "Fahrenheit",
            0x05 => "Ampere",
            0x06 => "Camdela",
            _ => "None",
        },
        (_, _) => "None",
    };

    let sys = data & 0xf;
    data >>= 4;

    if sys > 4 {
        if sys == 0xf {
            println!("{:indent$}System: Vendor defined, Unit: (unknown)", "");
        } else {
            println!("{:indent$}System: Reserved, Unit: (unknown)", "");
        }

        return;
    }

    print!("{:indent$}System: {}, Unit: ", "", systems(sys));

    let mut earlier_unit = 0;

    for i in 1..len * 2 {
        let nibble = data & 0xf;
        data >>= 4;
        if nibble != 0 {
            if earlier_unit > 0 {
                print!("*");
            }
            print!("{}", units(sys, i));
            earlier_unit += 1;
            /* This is a _signed_ nibble(!) */
            if nibble != 1 {
                let mut val: i8 = (nibble as i8) & 0x7;
                if nibble & 0x08 != 0x00 {
                    val = -((0x7 & !val) + 1);
                }
                print!("^{}", val);
            }
        }
    }

    if earlier_unit == 0 {
        print!("(None)");
    }
    println!();
}

/// Dumps HID report data ported directly from lsusb - it's not pretty but works...
fn dump_report_desc(desc: &[u8], indent: usize) {
    // ported from lsusb - indented to 28 spaces for some reason...
    const REPORT_INDENT: usize = 12;
    let types = |t: u8| match t {
        0x00 => "Main",
        0x01 => "Global",
        0x02 => "Local",
        _ => "reserved",
    };

    dump_string(
        &format!("Report Descriptor: (length is {})", desc.len()),
        indent,
    );

    let mut i = 0;
    while i < desc.len() {
        let b = desc[i];
        let mut data = 0xffff;
        let mut hut = 0xff;
        let mut bsize = (b & 0x03) as usize;
        if bsize == 3 {
            bsize = 4;
        }
        let btype = b & (0x03 << 2);
        let btag = b & !0x03;

        // Item Header
        print!(
            "{:indent$}Item({:>6}): {}, data=",
            "",
            types(btype >> 2),
            names::report_tag(btag).unwrap_or_default(),
            indent = indent + 2
        );

        // Check for descriptor bounds
        if i + bsize >= desc.len() {
            println!("Error: Descriptor too short");
            break;
        }

        if bsize > 0 {
            print!(" [ ");
            data = 0;
            for j in 0..bsize {
                data |= (desc[i + 1 + j] as u32) << (j * 8);
                print!("{:02x} ", desc[i + 1 + j]);
            }
            println!("] {}", data);
        } else {
            println!("none");
        }

        match btag {
            // usage page
            0x04 => {
                hut = data as u8;
                if let Some(hut) = names::huts(hut) {
                    println!("{:indent$}{}", "", hut, indent = REPORT_INDENT);
                }
            }
            // usage, usage minimum, usage maximum
            0x08 | 0x18 | 0x28 => {
                if let Some(hutus) = names::hutus(hut, data as u16) {
                    println!("{:indent$}{}", "", hutus, indent = REPORT_INDENT);
                }
            }
            // unit exponent
            0x54 => {
                println!(
                    "{:indent$}Unit Exponent: {}",
                    "",
                    data as u8,
                    indent = REPORT_INDENT
                );
            }
            // unit
            0x64 => dump_unit(data as u16, bsize, REPORT_INDENT),
            // collection
            0xa0 => match data {
                0x00 => println!("{:indent$}Physical", "", indent = REPORT_INDENT),
                0x01 => println!("{:indent$}Application", "", indent = REPORT_INDENT),
                0x02 => println!("{:indent$}Logical", "", indent = REPORT_INDENT),
                0x03 => println!("{:indent$}Report", "", indent = REPORT_INDENT),
                0x04 => println!("{:indent$}Named Array", "", indent = REPORT_INDENT),
                0x05 => println!("{:indent$}Usage Switch", "", indent = REPORT_INDENT),
                0x06 => println!("{:indent$}Usage Modifier", "", indent = REPORT_INDENT),
                _ => {
                    if (data & 0x80) == 0x80 {
                        println!("{:indent$}Vendor defined", "", indent = REPORT_INDENT)
                    } else {
                        println!("{:indent$}Unknown", "", indent = REPORT_INDENT)
                    }
                }
            },
            // input, output, feature
            0x80 | 0x90 | 0xb0 => {
                let attributes_1 = format!(
                    "{:indent$}{} {} {} {} {}",
                    "",
                    if data & 0x01 != 0 { "Constant" } else { "Data" },
                    if data & 0x02 != 0 {
                        "Variable"
                    } else {
                        "Array"
                    },
                    if data & 0x04 != 0 {
                        "Relative"
                    } else {
                        "Absolute"
                    },
                    if data & 0x08 != 0 { "Wrap" } else { "No_Wrap" },
                    if data & 0x10 != 0 {
                        "Non_Linear"
                    } else {
                        "Linear"
                    },
                    indent = REPORT_INDENT
                );

                let attributes_2 = format!(
                    "{:indent$}{} {} {} {}",
                    "",
                    if data & 0x20 != 0 {
                        "No_Preferred_State"
                    } else {
                        "Preferred_State"
                    },
                    if data & 0x40 != 0 {
                        "Null_State"
                    } else {
                        "No_Null_Position"
                    },
                    if data & 0x80 != 0 {
                        "Volatile"
                    } else {
                        "Non_Volatile"
                    },
                    if data & 0x100 != 0 {
                        "Buffered Bytes"
                    } else {
                        "Bitfield"
                    },
                    indent = REPORT_INDENT
                );
                println!("{}", attributes_1);
                println!("{}", attributes_2);
            }
            _ => (),
        }
        i += 1 + bsize;
    }
}
