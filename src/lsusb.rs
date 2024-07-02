//! Originally based on [libusb list_devices.rs example](https://github.com/dcuddeback/libusb-rs/blob/master/examples/list_devices.rs), attempts to mimic lsusb output and provide cross-platform [`crate::system_profiler::SPUSBDataType`] getter
//! Printing functions for lsusb style output of USB data
//!
//! The [lsusb source code](https://github.com/gregkh/usbutils/blob/master/lsusb.c) was used as a reference for a lot of the styling and content of the display module
//!
//! TODO:
//! - [ ] Implement do_otg: https://github.com/gregkh/usbutils/blob/master/lsusb.c#L3036
//! - [ ] Implement do_hub: https://github.com/gregkh/usbutils/blob/master/lsusb.c#L2805
//! - [ ] Implement do_debug: https://github.com/gregkh/usbutils/blob/master/lsusb.c#L2984
//! - [ ] Implement do_dualspeed: https://github.com/gregkh/usbutils/blob/master/lsusb.c#L2933
//! - [ ] Convert the 'in dump' descriptor decoding into concrete structs in [`crate::usb::descriptors`] and use that for printing - like the [`crate::usb::descriptors::audio`] module
use crate::display::PrintSettings;
use crate::error::{Error, ErrorKind};
use crate::system_profiler;

use crate::usb::descriptors::audio::*;
use crate::usb::descriptors::video::*;
use crate::usb::descriptors::*;
use crate::usb::*;

pub mod names;

const TREE_LSUSB_BUS: &str = "/:  ";
const TREE_LSUSB_DEVICE: &str = "|__ ";
const TREE_LSUSB_SPACE: &str = "    ";

const LSUSB_DUMP_WIDTH: usize = 24;
const LSUSB_DUMP_INDENT_BASE: usize = 2;

// TODO - convert these to Rust enum like [`Uac1ChannelNames`] etc.
const CAM_CTRL_NAMES: [&str; 22] = [
    "Scanning Mode",
    "Auto-Exposure Mode",
    "Auto-Exposure Priority",
    "Exposure Time (Absolute)",
    "Exposure Time (Relative)",
    "Focus (Absolute)",
    "Focus (Relative)",
    "Iris (Absolute)",
    "Iris (Relative)",
    "Zoom (Absolute)",
    "Zoom (Relative)",
    "PanTilt (Absolute)",
    "PanTilt (Relative)",
    "Roll (Absolute)",
    "Roll (Relative)",
    "Reserved",
    "Reserved",
    "Focus, Auto",
    "Privacy",
    "Focus, Simple",
    "Window",
    "Region of Interest",
];

const CTRL_NAMES: [&str; 19] = [
    "Brightness",
    "Contrast",
    "Hue",
    "Saturation",
    "Sharpness",
    "Gamma",
    "White Balance Temperature",
    "White Balance Component",
    "Backlight Compensation",
    "Gain",
    "Power Line Frequency",
    "Hue, Auto",
    "White Balance Temperature, Auto",
    "White Balance Component, Auto",
    "Digital Multiplier",
    "Digital Multiplier Limit",
    "Analog Video Standard",
    "Analog Video Lock Status",
    "Contrast, Auto",
];

const EN_CTRL_NAMES: [&str; 22] = [
    "Scanning Mode",
    "Auto-Exposure Mode",
    "Auto-Exposure Priority",
    "Exposure Time (Absolute)",
    "Exposure Time (Relative)",
    "Focus (Absolute)",
    "Focus (Relative)",
    "Iris (Absolute)",
    "Iris (Relative)",
    "Zoom (Absolute)",
    "Zoom (Relative)",
    "PanTilt (Absolute)",
    "PanTilt (Relative)",
    "Roll (Absolute)",
    "Roll (Relative)",
    "Reserved",
    "Reserved",
    "Focus, Auto",
    "Privacy",
    "Focus, Simple",
    "Window",
    "Region of Interest",
];

const STD_NAMES: [&str; 6] = [
    "None",
    "NTSC - 525/60",
    "PAL - 625/50",
    "SECAM - 625/50",
    "NTSC - 625/50",
    "PAL - 525/60",
];

const UAC2_INTERFACE_HEADER_BMCONTROLS: [&str; 1] = ["Legacy"];
const UAC2_INPUT_TERMINAL_BMCONTROLS: [&str; 6] = [
    "Copy Protect",
    "Connector",
    "Overload",
    "Cluster",
    "Underflow",
    "Overflow",
];
const UAC3_INPUT_TERMINAL_BMCONTROLS: [&str; 5] = [
    "Insertion",
    "Overload",
    "Underflow",
    "Overflow",
    "Underflow",
];
const UAC2_OUTPUT_TERMINAL_BMCONTROLS: [&str; 5] = [
    "Copy Protect",
    "Connector",
    "Overload",
    "Underflow",
    "Overflow",
];
const UAC3_OUTPUT_TERMINAL_BMCONTROLS: [&str; 4] =
    ["Insertion", "Overload", "Underflow", "Overflow"];
const UAC2_AS_INTERFACE_BMCONTROLS: [&str; 2] =
    ["Active Alternate Setting", "Valid Alternate Setting"];
const UAC3_AS_INTERFACE_BMCONTROLS: [&str; 3] = [
    "Active Alternate Setting",
    "Valid Alternate Setting",
    "Audio Data Format Control",
];
const UAC2_AS_ISO_ENDPOINT_BMCONTROLS: [&str; 3] = ["Pitch", "Data Overrun", "Data Underrun"];
const UAC2_MIXER_UNIT_BMCONTROLS: [&str; 4] = ["Cluster", "Underflow", "Overflow", "Overflow"];
const UAC3_MIXER_UNIT_BMCONTROLS: [&str; 2] = ["Underflow", "Overflow"];
const UAC2_SELECTOR_UNIT_BMCONTROLS: [&str; 1] = ["Selector"];
const UAC1_FEATURE_UNIT_BMCONTROLS: [&str; 13] = [
    "Mute",
    "Volume",
    "Bass",
    "Mid",
    "Treble",
    "Graphic Equalizer",
    "Automatic Gain",
    "Delay",
    "Bass Boost",
    "Loudness",
    "Input gain",
    "Input gain pad",
    "Phase invert",
];
const UAC2_EXTENSION_UNIT_BMCONTROLS: [&str; 4] = ["Enable", "Cluster", "Underflow", "Overflow"];
const UAC3_EXTENSION_UNIT_BMCONTROLS: [&str; 2] = ["Underflow", "Overflow"];
const UAC2_CLOCK_SOURCE_BMCONTROLS: [&str; 2] = ["Clock Frequency", "Clock Validity"];
const UAC2_CLOCK_SELECTOR_BMCONTROLS: [&str; 1] = ["Clock Selector"];
const UAC2_CLOCK_MULTIPLIER_BMCONTROLS: [&str; 2] = ["Clock Numerator", "Clock Denominator"];
const UAC3_PROCESSING_UNIT_UP_DOWN_BMCONTROLS: [&str; 3] = ["Mode Select", "Underflow", "Overflow"];
const UAC3_PROCESSING_UNIT_STEREO_EXTENDER_BMCONTROLS: [&str; 3] =
    ["Width", "Underflow", "Overflow"];
const UAC3_PROCESSING_UNIT_MULTI_FUNC_BMCONTROLS: [&str; 2] = ["Underflow", "Overflow"];

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

/// Dump just indented title
fn dump_title(field_name: &str, indent: usize) {
    println!("{:indent$}{}", "", field_name, indent = indent);
}

/// Dump a single value like lsusb
fn dump_value<T: std::fmt::Display>(value: T, field_name: &str, indent: usize, width: usize) {
    let value = value.to_string();
    let spaces = " ".repeat(
        (width - value.len())
            .saturating_sub(field_name.len())
            .max(1),
    );
    println!(
        "{:indent$}{}{}{}",
        "",
        field_name,
        spaces,
        value,
        indent = indent
    );
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
    let dump = format!(
        "{:indent$}{}{}{}",
        "",
        field_name,
        spaces,
        value_string,
        indent = indent
    );
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
        "",
        field_name,
        spaces,
        value,
        value_string,
        indent = indent
    );
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

/// Dumps strings matching the bits set in `bitmap` using `strings_f` function
fn dump_bitmap_strings<T>(bitmap: T, strings_f: fn(usize) -> Option<&'static str>, indent: usize)
where
    T: std::fmt::Display + std::fmt::LowerHex + Copy + Into<u64>,
{
    let bitmap_u64: u64 = bitmap.into();
    let num_bits = std::mem::size_of::<T>() * 8;
    for index in 0..num_bits {
        if (bitmap_u64 >> index) & 0x1 != 0 {
            if let Some(string) = strings_f(index) {
                println!("{:indent$}{}", "", string, indent = indent);
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
    print!(
        "{:indent$}{}{}{}",
        "",
        field_name,
        spaces,
        value,
        indent = indent
    );
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
                    print_device(device);

                    for config in &device_extra.configurations {
                        print_config(config, LSUSB_DUMP_INDENT_BASE);

                        for interface in &config.interfaces {
                            print_interface(interface, LSUSB_DUMP_INDENT_BASE * 2);

                            for endpoint in &interface.endpoints {
                                print_endpoint(endpoint, LSUSB_DUMP_INDENT_BASE * 3);
                            }
                        }
                    }
                }
            }
        }
    }
}

fn print_device(device: &system_profiler::USBDevice) {
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

fn print_config(config: &USBConfiguration, indent: usize) {
    dump_title("Configuration Descriptor:", indent);
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
        dump_title("(Bus Powered)", indent + 4);
    } else {
        if config.attributes.contains(&ConfigAttributes::SelfPowered) {
            dump_title("Self Powered", indent + 4);
        }
        if config.attributes.contains(&ConfigAttributes::RemoteWakeup) {
            dump_title("Remote Wakeup", indent + 4);
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

fn print_interface(interface: &USBInterface, indent: usize) {
    let interface_name = names::class(interface.class.into());
    let sub_class_name = names::subclass(interface.class.into(), interface.sub_class);
    let protocol_name = names::protocol(
        interface.class.into(),
        interface.sub_class,
        interface.protocol,
    );

    dump_title("Interface Descriptor:", indent);
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
                    ClassDescriptor::Midi(md, _) => dump_midistreaming_interface(md),
                    ClassDescriptor::Audio(uacd, uacp) => match &uacd.subtype {
                        UacType::Control(cs) => dump_audiocontrol_interface(uacd, cs, uacp),
                        UacType::Streaming(ss) => dump_audiostreaming_interface(uacd, ss, uacp),
                        _ => (),
                    },
                    ClassDescriptor::Video(vcd, p) => dump_videocontrol_interface(vcd, *p),
                    ClassDescriptor::Generic(cc, gd) => match cc {
                        Some((ClassCode::Audio, 3, _)) => {
                            if let Ok(md) = MidiDescriptor::try_from(gd.to_owned()) {
                                dump_midistreaming_interface(&md);
                            }
                        }
                        Some((ClassCode::Audio, s, p)) => {
                            if let Ok(uacd) = UacDescriptor::try_from((gd.to_owned(), *s, *p)) {
                                let uacp = UacProtocol::from(*p);
                                match &uacd.subtype {
                                    UacType::Control(cs) => {
                                        dump_audiocontrol_interface(&uacd, cs, &uacp)
                                    }
                                    UacType::Streaming(ss) => {
                                        dump_audiostreaming_interface(&uacd, ss, &uacp)
                                    }
                                    _ => (),
                                }
                            }
                        }
                        Some((ClassCode::Video, 1, p)) => {
                            if let Ok(vcd) = UvcDescriptor::try_from(gd.to_owned()) {
                                dump_videocontrol_interface(&vcd, *p);
                            }
                        }
                        Some((ClassCode::Video, 2, _)) => {
                            dump_videostreaming_interface(gd);
                        }
                        Some((ClassCode::ApplicationSpecificInterface, 1, _)) => {
                            dump_dfu_interface(gd, indent + 2);
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

fn print_endpoint(endpoint: &USBEndpoint, indent: usize) {
    dump_title("Endpoint Descriptor:", indent);
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
                            if let Ok(uacd) = UacDescriptor::try_from((gd.to_owned(), 2, *p)) {
                                dump_audiostreaming_endpoint(&uacd, indent + 2);
                            }
                        }
                        Some((ClassCode::Audio, 3, _)) => {
                            dump_midistreaming_endpoint(gd, indent + 2);
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

fn dump_audiostreaming_endpoint(ad: &UacDescriptor, indent: usize) {
    // audio streaming endpoint is only EP_GENERAL
    let subtype_string = match ad.subtype {
        UacType::Streaming(StreamingSubtype::General) => "EP_GENERAL",
        // lowercase in lsusb
        _ => "invalid",
    };
    dump_title("AudioStreaming Endpoint Descriptor:", indent);
    dump_value(ad.length, "bLength", indent + 2, LSUSB_DUMP_WIDTH);
    dump_value(ad.descriptor_type, "bDescriptorType", indent + 2, LSUSB_DUMP_WIDTH);
    dump_value_string(
        u8::from(ad.subtype.to_owned()),
        "bDescriptorSubtype",
        format!("({:#})", subtype_string),
        indent + 2,
        LSUSB_DUMP_WIDTH,
    );

    if matches!(ad.subtype, UacType::Streaming(StreamingSubtype::General)) {
        dump_audio_subtype(&ad.interface, indent + 2);
    }
}

fn dump_midistreaming_endpoint(gd: &GenericDescriptor, indent: usize) {
    let subtype_string = match gd.descriptor_subtype {
        2 => "GENERAL",
        _ => "Invalid",
    };

    dump_title("MIDIStreaming Endpoint Descriptor:", indent);
    dump_value(gd.length, "bLength", indent + 2, LSUSB_DUMP_WIDTH);
    dump_value(gd.descriptor_type, "bDescriptorType", indent + 2, LSUSB_DUMP_WIDTH);
    dump_value_string(gd.descriptor_subtype, subtype_string, "bDescriptorSubtype", indent + 2, LSUSB_DUMP_WIDTH);

    if let Some(data) = gd.data.as_ref() {
        if data.len() >= 2 {
            let num_jacks = data[0] as usize;
            dump_value(num_jacks, "bNumEmbMIDIJack", indent + 2, LSUSB_DUMP_WIDTH);
            if data.len() >= num_jacks {
                dump_array(&data[1..num_jacks], "baAssocJackID", indent + 2, LSUSB_DUMP_WIDTH);
            }
        }
        dump_junk(data, indent, gd.expected_data_length(), 1 + data[0] as usize);
    }
}

fn dump_ccid_desc(ccid: &CcidDescriptor, indent: usize) {
    dump_title("ChipCard Interface Descriptor:", indent);
    dump_value(ccid.length, "bLength", indent + 2, LSUSB_DUMP_WIDTH);
    dump_value(ccid.descriptor_type, "bDescriptorType", indent + 2, LSUSB_DUMP_WIDTH);
    dump_value(ccid.version, "bcdCCID", indent + 2, LSUSB_DUMP_WIDTH);
    if ccid.version.major() != 1 || ccid.version.minor() != 0 {
        println!("  (Warning: Only accurate for version 1.0)");
    }

    dump_value(ccid.max_slot_index, "bMaxSlotIndex", indent + 2, LSUSB_DUMP_WIDTH);
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

    dump_value(ccid.default_clock, "dwDefaultClock", indent + 2, LSUSB_DUMP_WIDTH);
    dump_value(ccid.max_clock, "dwMaxiumumClock", indent + 2, LSUSB_DUMP_WIDTH);
    dump_value(ccid.num_clock_supported, "bNumClockSupported", indent + 2, LSUSB_DUMP_WIDTH);
    dump_value_string(ccid.data_rate, "dwDataRate", "bps", indent + 2, LSUSB_DUMP_WIDTH);
    dump_value_string(ccid.max_data_rate, "dwMaxDataRate", "bps", indent + 2, LSUSB_DUMP_WIDTH);
    dump_value(ccid.num_data_rates_supp, "bNumDataRatesSupp.", indent + 2, LSUSB_DUMP_WIDTH);
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

    dump_value(format!("{:08X}", ccid.features), "dwFeatures", indent + 2, LSUSB_DUMP_WIDTH);
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
        indent + 2
    );
    if (ccid.features & (0x0040 | 0x0080)) != 0 {
        println!("{:indent$}WARNING: conflicting negotiation features", "", indent = indent + 2);
    }
    if ccid.features & 0x00070000 != 0 {
        println!("{:indent$}WARNING: conflicting exchange levels", "", indent = indent + 2);
    }

    dump_value(ccid.max_ccid_msg_len, "dwMaxCCIDMsgLen", indent + 2, LSUSB_DUMP_WIDTH);
    if ccid.class_get_response == 0xff {
        dump_value("echo", "bClassGetResponse", indent + 2, LSUSB_DUMP_WIDTH);
    } else {
        dump_value(format!("{:02X}", ccid.class_get_response), "bClassGetResponse", indent + 2, LSUSB_DUMP_WIDTH);
    }

    if ccid.class_envelope == 0xff {
        dump_value("echo", "bClassEnvelope", indent + 2, LSUSB_DUMP_WIDTH);
    } else {
        dump_value(format!("{:02X}", ccid.class_envelope), "bClassEnvelope", indent + 2, LSUSB_DUMP_WIDTH);
    }

    if ccid.lcd_layout == (0, 0) {
        dump_value("none", "wlcdLayout", indent + 2, LSUSB_DUMP_WIDTH);
    } else {
        dump_value_string(ccid.lcd_layout.0, "wlcdLayout", format!(" cols {} lines", ccid.lcd_layout.1), indent + 2, LSUSB_DUMP_WIDTH);
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

    dump_value(ccid.max_ccid_busy_slots, "bMaxCCIDBusySlots", indent + 2, LSUSB_DUMP_WIDTH);
}

fn dump_printer_desc(pd: &PrinterDescriptor, indent: usize) {
    dump_title("Printer Interface Descriptor:", indent);
    dump_value(pd.length, "bLength", indent + 2, LSUSB_DUMP_WIDTH);
    dump_value(pd.descriptor_type, "bDescriptorType", indent + 2, LSUSB_DUMP_WIDTH);
    dump_value(pd.release_number, "bcdReleaseNumber", indent + 2, LSUSB_DUMP_WIDTH);
    dump_value(pd.descriptors.len(), "bcdNumDescriptors", indent + 2, LSUSB_DUMP_WIDTH);

    for desc in &pd.descriptors {
        // basic capabilities
        if desc.descriptor_type == 0x00 {
            dump_value(desc.versions_supported, "iIPPVersionsSupported", indent + 2, LSUSB_DUMP_WIDTH);
            dump_value_string(desc.uuid_string_index, "iIPPPrinterUUID", desc.uuid_string.as_ref().unwrap_or(&String::new()), indent + 2, LSUSB_DUMP_WIDTH);  
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
            dump_value_string(desc.descriptor_type, "UnknownCapabilities", desc.length, indent + 2, LSUSB_DUMP_WIDTH);
        }
    }
}

fn dump_bmcontrols<T: Into<u32>>(
    controls: T,
    control_descriptions: &[&'static str],
    desc_type: &ControlType,
    indent: usize,
) {
    let controls: u32 = controls.into();
    for (index, control) in control_descriptions.iter().enumerate() {
        match desc_type {
            ControlType::BmControl1 => {
                if (controls >> index) & 0x1 != 0 {
                    println!("{:indent$}{} Control", "", control, indent = indent);
                }
            }
            ControlType::BmControl2 => {
                println!(
                    "{:indent$}{} Control ({})",
                    "",
                    control,
                    ControlSetting::from(((controls >> (index * 2)) & 0x3) as u8),
                    indent = indent
                )
            }
        }
    }
}

fn dump_bmcontrols_array<T: Into<u32> + std::fmt::Display + Copy>(
    field_name: &str,
    controls: &[T],
    control_descriptions: &[&'static str],
    desc_type: &ControlType,
    indent: usize,
    width: usize,
) {
    for (i, control) in controls.iter().enumerate() {
        let control = control.to_owned();
        let control: u32 = control.into();
        dump_value(control, &format!("{}({:2})", field_name, i), indent, width);
        dump_bmcontrols(control, control_descriptions, desc_type, indent + 2);
    }
}

fn dump_audio_mixer_unit1(mixer_unit: &MixerUnit1, indent: usize, width: usize) {
    dump_value(mixer_unit.unit_id, "bUnitID", indent, width);
    dump_value(mixer_unit.nr_in_pins, "bNrInPins", indent, width);
    dump_array(&mixer_unit.source_ids, "baSourceID", indent, width);
    dump_value(mixer_unit.nr_channels, "bNrChannels", indent, width);
    dump_hex(mixer_unit.channel_config, "wChannelConfig", indent, width);
    let channel_names = UacInterfaceDescriptor::get_channel_name_strings(
        &UacProtocol::Uac1,
        mixer_unit.channel_config as u32,
    );
    for name in channel_names.iter() {
        println!("{:indent$}{}", "", name, indent = indent + 2);
    }
    dump_value(mixer_unit.channel_names, "iChannelNames", indent, width);
    dump_bitmap_array(&mixer_unit.controls, "bmControls", indent, width);
    dump_value(mixer_unit.mixer, "iMixer", indent, width);
}

fn dump_audio_mixer_unit2(mixer_unit: &MixerUnit2, indent: usize, width: usize) {
    dump_value(mixer_unit.unit_id, "bUnitID", indent, width);
    dump_value(mixer_unit.nr_in_pins, "bNrInPins", indent, width);
    dump_array(&mixer_unit.source_ids, "baSourceID", indent, width);
    dump_value(mixer_unit.nr_channels, "bNrChannels", indent, width);
    dump_hex(mixer_unit.channel_config, "bmChannelConfig", indent, width);
    let channel_names = UacInterfaceDescriptor::get_channel_name_strings(
        &UacProtocol::Uac2,
        mixer_unit.channel_config,
    );
    for name in channel_names.iter() {
        println!("{:indent$}{}", "", name, indent = indent + 2);
    }
    dump_value(mixer_unit.channel_names, "iChannelNames", indent, width);
    dump_bitmap_array(&mixer_unit.mixer_controls, "bmMixerControls", indent, width);
    dump_hex(mixer_unit.controls, "bmControls", indent, width);
    dump_bmcontrols(
        mixer_unit.controls as u32,
        &UAC2_MIXER_UNIT_BMCONTROLS,
        &ControlType::BmControl2,
        indent + 2,
    );
    dump_value(mixer_unit.mixer, "iMixer", indent, width);
}

fn dump_audio_mixer_unit3(mixer_unit: &MixerUnit3, indent: usize, width: usize) {
    dump_value(mixer_unit.unit_id, "bUnitID", indent, width);
    dump_value(mixer_unit.nr_in_pins, "bNrInPins", indent, width);
    dump_array(&mixer_unit.source_ids, "baSourceID", indent, width);
    dump_value(
        mixer_unit.cluster_descr_id,
        "wClusterDescrID",
        indent,
        width,
    );
    dump_bitmap_array(&mixer_unit.mixer_controls, "bmMixerControls", indent, width);
    dump_hex(mixer_unit.controls, "bmControls", indent, width);
    dump_bmcontrols(
        mixer_unit.controls,
        &UAC3_MIXER_UNIT_BMCONTROLS,
        &ControlType::BmControl2,
        indent + 2,
    );
    dump_value(mixer_unit.mixer_descr_str, "wMixerDescrStr", indent, width);
}

fn dump_audio_power_domain(power_domain: &PowerDomain, indent: usize, width: usize) {
    dump_value(
        power_domain.power_domain_id,
        "bPowerDomainID",
        indent,
        width,
    );
    dump_value(
        power_domain.recovery_time_1,
        "waRecoveryTime(1)",
        indent,
        width,
    );
    dump_value(
        power_domain.recovery_time_2,
        "waRecoveryTime(2)",
        indent,
        width,
    );
    dump_value(power_domain.nr_entities, "bNrEntities", indent, width);
    dump_array(&power_domain.entity_ids, "baEntityID", indent, width);
    dump_value(
        power_domain.domain_descr_str,
        "wPDomainDescrStr",
        indent,
        width,
    );
}

fn dump_audio_selector_unit1(selector_unit: &SelectorUnit1, indent: usize, width: usize) {
    dump_value(selector_unit.unit_id, "bUnitID", indent, width);
    dump_value(selector_unit.nr_in_pins, "bNrInPins", indent, width);
    dump_array(&selector_unit.source_ids, "baSourceID", indent, width);
    dump_value_string(
        selector_unit.selector_index,
        "iSelector",
        selector_unit.selector.as_ref().unwrap_or(&"".into()),
        indent,
        width,
    );
}

fn dump_audio_selector_unit2(selector_unit: &SelectorUnit2, indent: usize, width: usize) {
    dump_value(selector_unit.unit_id, "bUnitID", indent, width);
    dump_value(selector_unit.nr_in_pins, "bNrInPins", indent, width);
    dump_array(&selector_unit.source_ids, "baSourceID", indent, width);
    dump_hex(selector_unit.controls, "bmControls", indent, width);
    dump_bmcontrols(
        selector_unit.controls,
        &UAC2_SELECTOR_UNIT_BMCONTROLS,
        &ControlType::BmControl2,
        indent + 2,
    );
    dump_value_string(
        selector_unit.selector_index,
        "iSelector",
        selector_unit.selector.as_ref().unwrap_or(&"".into()),
        indent,
        width,
    );
}

fn dump_audio_selector_unit3(selector_unit: &SelectorUnit3, indent: usize, width: usize) {
    dump_value(selector_unit.unit_id, "bUnitID", indent, width);
    dump_value(selector_unit.nr_in_pins, "bNrInPins", indent, width);
    dump_array(&selector_unit.source_ids, "baSourceID", indent, width);
    dump_hex(selector_unit.controls, "bmControls", indent, width);
    dump_bmcontrols(
        selector_unit.controls,
        &UAC2_SELECTOR_UNIT_BMCONTROLS,
        &ControlType::BmControl2,
        indent + 2,
    );
    dump_value(
        selector_unit.selector_descr_str,
        "wSelectorDescrStr",
        indent,
        width,
    );
}

/// Dumps the contents of a UAC1 Processing Unit Descriptor
fn dump_audio_processing_unit1(unit: &ProcessingUnit1, indent: usize, width: usize) {
    dump_value(unit.unit_id, "bUnitID", indent, width);
    dump_value_string(
        unit.process_type,
        "wProcessType",
        unit.processing_type(),
        indent,
        width,
    );
    dump_value(unit.nr_in_pins, "bNrInPins", indent, width);
    dump_array(&unit.source_ids, "baSourceID", indent, width);
    dump_value(unit.nr_channels, "bNrChannels", indent, width);
    dump_hex(unit.channel_config, "wChannelConfig", indent, width);
    let channel_names = UacInterfaceDescriptor::get_channel_name_strings(
        &UacProtocol::Uac1,
        unit.channel_config as u32,
    );
    for name in channel_names.iter() {
        println!("{:indent$}{}", "", name, indent = indent + 2);
    }
    dump_value_string(
        unit.channel_names_index,
        "iChannelNames",
        unit.channel_names.as_ref().unwrap_or(&"".into()),
        indent,
        width,
    );
    dump_value(unit.control_size, "bControlSize", indent, width);
    dump_bitmap_array(&unit.controls, "bmControls", indent, width);
    dump_value_string(
        unit.processing_index,
        "iProcessing",
        unit.processing.as_ref().unwrap_or(&"".into()),
        indent,
        width,
    );
    if let Some(ref specific) = unit.specific {
        dump_value(specific.nr_modes, "bNrModes", indent, width);
        dump_bitmap_array(&specific.modes, "waModes", indent, width);
    }
}

/// Dumps the contents of a UAC2 Processing Unit Descriptor
fn dump_audio_processing_unit2(unit: &ProcessingUnit2, indent: usize, width: usize) {
    dump_value(unit.unit_id, "bUnitID", indent, width);
    dump_value_string(
        unit.process_type,
        "wProcessType",
        unit.processing_type(),
        indent,
        width,
    );
    dump_value(unit.nr_in_pins, "bNrInPins", indent, width);
    dump_array(&unit.source_ids, "baSourceID", indent, width);
    dump_value(unit.nr_channels, "bNrChannels", indent, width);
    dump_hex(unit.channel_config, "bmChannelConfig", indent, width);
    let channel_names =
        UacInterfaceDescriptor::get_channel_name_strings(&UacProtocol::Uac2, unit.channel_config);
    for name in channel_names.iter() {
        println!("{:indent$}{}", "", name, indent = indent + 2);
    }
    dump_value_string(
        unit.channel_names_index,
        "iChannelNames",
        unit.channel_names.as_ref().unwrap_or(&"".into()),
        indent,
        width,
    );
    dump_value(unit.controls, "bmControls", indent, width);
    dump_value_string(
        unit.processing_index,
        "iProcessing",
        unit.processing.as_ref().unwrap_or(&"".into()),
        indent,
        width,
    );
    if let Some(ref specific) = unit.specific {
        match specific {
            AudioProcessingUnit2Specific::UpDownMix(up_down_mix) => {
                dump_value(up_down_mix.nr_modes, "bNrModes", indent, width);
                dump_bitmap_array(&up_down_mix.modes, "daModes", indent, width);
            }
            AudioProcessingUnit2Specific::DolbyPrologic(dolby_prologic) => {
                dump_value(dolby_prologic.nr_modes, "bNrModes", indent, width);
                dump_bitmap_array(&dolby_prologic.modes, "daModes", indent, width);
            }
        }
    }
}

/// Dumps the contents of a UAC3 Processing Unit Descriptor
fn dump_audio_processing_unit3(unit: &ProcessingUnit3, indent: usize, width: usize) {
    dump_value(unit.unit_id, "bUnitID", indent, width);
    dump_value_string(
        unit.process_type,
        "wProcessType",
        unit.processing_type(),
        indent,
        width,
    );
    dump_value(unit.nr_in_pins, "bNrInPins", indent, width);
    dump_array(&unit.source_ids, "baSourceID", indent, width);
    dump_value(
        unit.processing_descr_str,
        "wProcessingDescrStr",
        indent,
        width,
    );
    if let Some(ref specific) = unit.specific {
        match specific {
            AudioProcessingUnit3Specific::UpDownMix(up_down_mix) => {
                dump_hex(up_down_mix.controls, "bmControls", indent, width);
                dump_bmcontrols(
                    up_down_mix.controls,
                    &UAC3_PROCESSING_UNIT_UP_DOWN_BMCONTROLS,
                    &ControlType::BmControl2,
                    indent + 2,
                );
                dump_value(up_down_mix.nr_modes, "bNrModes", indent, width);
                dump_array(
                    &up_down_mix.cluster_descr_ids,
                    "waClusterDescrID",
                    indent,
                    width,
                );
            }
            AudioProcessingUnit3Specific::StereoExtender(stereo_extender) => {
                dump_hex(stereo_extender.controls, "bmControls", indent, width);
                dump_bmcontrols(
                    stereo_extender.controls,
                    &UAC3_PROCESSING_UNIT_STEREO_EXTENDER_BMCONTROLS,
                    &ControlType::BmControl2,
                    indent + 2,
                );
            }
            AudioProcessingUnit3Specific::MultiFunction(multi_function) => {
                dump_hex(multi_function.controls, "bmControls", indent, width);
                dump_bmcontrols(
                    multi_function.controls,
                    &UAC3_PROCESSING_UNIT_MULTI_FUNC_BMCONTROLS,
                    &ControlType::BmControl2,
                    indent + 2,
                );
                dump_value(
                    multi_function.cluster_descr_id,
                    "wClusterDescrID",
                    indent,
                    width,
                );
                dump_value(multi_function.algorithms, "bmAlgorithms", indent, width);
                if let Some(ref algorithms) = unit.algorithms() {
                    for algorithm in algorithms.iter() {
                        println!("{:indent$}{}", "", algorithm, indent = indent + 2);
                    }
                }
            }
        }
    }
}

/// Dumps the contents of a UAC2 Effect Unit Descriptor
fn dump_audio_effect_unit2(unit: &EffectUnit2, indent: usize, width: usize) {
    dump_value(unit.unit_id, "bUnitID", indent, width);
    dump_value(unit.effect_type, "wEffectType", indent, width);
    dump_value(unit.source_id, "bSourceID", indent, width);
    dump_bitmap_array(&unit.controls, "bmaControls", indent, width);
    dump_value(unit.effect_index, "iEffects", indent, width);
    dump_value_string(
        unit.effect_index,
        "iEffects",
        unit.effect.as_ref().unwrap_or(&"".into()),
        indent,
        width,
    );
}

/// Dumps the contents of a UAC3 Effect Unit Descriptor
fn dump_audio_effect_unit3(unit: &EffectUnit3, indent: usize, width: usize) {
    dump_value(unit.unit_id, "bUnitID", indent, width);
    dump_value(unit.effect_type, "wEffectType", indent, width);
    dump_value(unit.source_id, "bSourceID", indent, width);
    dump_bitmap_array(&unit.controls, "bmaControls", indent, width);
    dump_value(unit.effect_descr_str, "wEffectsDescrStr", indent, width);
}

/// Dumps the contents of a UAC1 Feature Unit Descriptor
fn dump_audio_feature_unit1(unit: &FeatureUnit1, indent: usize, width: usize) {
    dump_value(unit.unit_id, "bUnitID", indent, width);
    dump_value(unit.source_id, "bSourceID", indent, width);
    dump_value(unit.control_size, "bControlSize", indent, width);
    dump_bmcontrols_array(
        "bmaControls",
        &unit.controls,
        &UAC1_FEATURE_UNIT_BMCONTROLS,
        &ControlType::BmControl1,
        indent,
        width,
    );
    dump_value_string(
        unit.feature_index,
        "iFeature",
        unit.feature.as_ref().unwrap_or(&"".into()),
        indent,
        width,
    );
}

/// Dumps the contents of a UAC2 Feature Unit Descriptor
fn dump_audio_feature_unit2(unit: &FeatureUnit2, indent: usize, width: usize) {
    dump_value(unit.unit_id, "bUnitID", indent, width);
    dump_value(unit.source_id, "bSourceID", indent, width);
    dump_bmcontrols_array(
        "bmaControls",
        &unit.controls,
        &UAC1_FEATURE_UNIT_BMCONTROLS,
        &ControlType::BmControl1,
        indent,
        width,
    );
    dump_value_string(
        unit.feature_index,
        "iFeature",
        unit.feature.as_ref().unwrap_or(&"".into()),
        indent,
        width,
    );
}

/// Dumps the contents of a UAC3 Feature Unit Descriptor
fn dump_audio_feature_unit3(unit: &FeatureUnit3, indent: usize, width: usize) {
    dump_value(unit.unit_id, "bUnitID", indent, width);
    dump_value(unit.source_id, "bSourceID", indent, width);
    dump_bmcontrols_array(
        "bmaControls",
        &unit.controls,
        &UAC1_FEATURE_UNIT_BMCONTROLS,
        &ControlType::BmControl1,
        indent,
        width,
    );
    dump_value(unit.feature_descr_str, "wFeatureDescrStr", indent, width);
}

/// Dumps the contents of a UAC1 Extension Unit Descriptor
fn dump_audio_extension_unit1(unit: &ExtensionUnit1, indent: usize, width: usize) {
    dump_value(unit.unit_id, "bUnitID", indent, width);
    dump_value(unit.extension_code, "wExtensionCode", indent, width);
    dump_value(unit.nr_in_pins, "bNrInPins", indent, width);
    dump_array(&unit.source_ids, "baSourceID", indent, width);
    dump_value(unit.nr_channels, "bNrChannels", indent, width);
    dump_hex(unit.channel_config, "wChannelConfig", indent, width);
    let channel_names = UacInterfaceDescriptor::get_channel_name_strings(
        &UacProtocol::Uac1,
        unit.channel_config as u32,
    );
    for name in channel_names.iter() {
        println!("{:indent$}{}", "", name, indent = indent + 2);
    }
    dump_value(unit.channel_names_index, "iChannelNames", indent, width);
    dump_value_string(
        unit.channel_names_index,
        "iChannelNames",
        unit.channel_names.as_ref().unwrap_or(&"".into()),
        indent,
        width,
    );
    dump_value(unit.control_size, "bControlSize", indent, width);
    dump_bitmap_array(&unit.controls, "bmControls", indent, width);
    dump_value_string(
        unit.extension_index,
        "iExtension",
        unit.extension.as_ref().unwrap_or(&"".into()),
        indent,
        width,
    );
}

/// Dumps the contents of a UAC2 Extension Unit Descriptor
fn dump_audio_extension_unit2(unit: &ExtensionUnit2, indent: usize, width: usize) {
    dump_value(unit.unit_id, "bUnitID", indent, width);
    dump_value(unit.extension_code, "wExtensionCode", indent, width);
    dump_value(unit.nr_in_pins, "bNrInPins", indent, width);
    dump_array(&unit.source_ids, "baSourceID", indent, width);
    dump_value(unit.nr_channels, "bNrChannels", indent, width);
    dump_hex(unit.channel_config, "bmChannelConfig", indent, width);
    let channel_names =
        UacInterfaceDescriptor::get_channel_name_strings(&UacProtocol::Uac2, unit.channel_config);
    for name in channel_names.iter() {
        println!("{:indent$}{}", "", name, indent = indent + 2);
    }
    dump_value_string(
        unit.channel_names_index,
        "iChannelNames",
        unit.channel_names.as_ref().unwrap_or(&"".into()),
        indent,
        width,
    );
    dump_hex(unit.controls, "bmControls", indent, width);
    dump_bmcontrols(
        unit.controls,
        &UAC2_EXTENSION_UNIT_BMCONTROLS,
        &ControlType::BmControl2,
        indent + 2,
    );
    dump_value_string(
        unit.extension_index,
        "iExtension",
        unit.extension.as_ref().unwrap_or(&"".into()),
        indent,
        width,
    );
}

/// Dumps the contents of a UAC3 Extension Unit Descriptor
fn dump_audio_extension_unit3(unit: &ExtensionUnit3, indent: usize, width: usize) {
    dump_value(unit.unit_id, "bUnitID", indent, width);
    dump_value(unit.extension_code, "wExtensionCode", indent, width);
    dump_value(unit.nr_in_pins, "bNrInPins", indent, width);
    dump_array(&unit.source_ids, "baSourceID", indent, width);
    dump_value(
        unit.extension_descr_str,
        "wExtensionDescrStr",
        indent,
        width,
    );
    dump_hex(unit.controls, "bmControls", indent, width);
    dump_bmcontrols(
        unit.controls,
        &UAC3_EXTENSION_UNIT_BMCONTROLS,
        &ControlType::BmControl2,
        indent + 2,
    );
    dump_value(unit.cluster_descr_id, "wClusterDescrID", indent, width);
}

/// Dumps the contents of a UAC2 Clock Source Descriptor
fn dump_audio_clock_source2(source: &ClockSource2, indent: usize, width: usize) {
    let uac2_clk_src_bmattr = |index: usize| -> Option<&'static str> {
        match index {
            0 => Some("External"),
            1 => Some("Internal fixed"),
            2 => Some("Internal variable"),
            3 => Some("Internal programmable"),
            _ => None,
        }
    };

    dump_value(source.clock_id, "bClockID", indent, width);
    dump_hex(source.attributes, "bmAttributes", indent, width);
    dump_bitmap_strings(source.attributes, uac2_clk_src_bmattr, indent + 2);
    dump_hex(source.controls, "bmControls", indent, width);
    dump_bmcontrols(
        source.controls,
        &UAC2_CLOCK_SOURCE_BMCONTROLS,
        &ControlType::BmControl2,
        indent + 2,
    );
    dump_value(source.assoc_terminal, "bAssocTerminal", indent, width);
    dump_value_string(
        source.clock_source_index,
        "iClockSource",
        source.clock_source.as_ref().unwrap_or(&"".into()),
        indent,
        width,
    );
}

/// Dumps the contents of a UAC3 Clock Source Descriptor
fn dump_audio_clock_source3(source: &ClockSource3, indent: usize, width: usize) {
    let uac3_clk_src_bmattr = |index: usize| -> Option<&'static str> {
        match index {
            0 => Some("External"),
            1 => Some("Internal"),
            2 => Some("(asynchronous)"),
            3 => Some("(synchronized to SOF)"),
            _ => None,
        }
    };

    dump_value(source.clock_id, "bClockID", indent, width);
    dump_hex(source.attributes, "bmAttributes", indent, width);
    dump_bitmap_strings(source.attributes, uac3_clk_src_bmattr, indent + 2);
    dump_hex(source.controls, "bmControls", indent, width);
    dump_bmcontrols(
        source.controls,
        &UAC2_CLOCK_SOURCE_BMCONTROLS,
        &ControlType::BmControl2,
        indent + 2,
    );
    dump_value(
        source.reference_terminal,
        "bReferenceTerminal",
        indent,
        width,
    );
    dump_value(source.clock_source_str, "wClockSourceStr", indent, width);
}

/// Dumps the contents of a UAC2 Clock Selector Descriptor
fn dump_audio_clock_selector2(selector: &ClockSelector2, indent: usize, width: usize) {
    dump_value(selector.clock_id, "bClockID", indent, width);
    dump_value(selector.nr_in_pins, "bNrInPins", indent, width);
    dump_array(&selector.csource_ids, "baCSourceID", indent, width);
    dump_hex(selector.controls, "bmControls", indent, width);
    dump_bmcontrols(
        selector.controls,
        &UAC2_CLOCK_SELECTOR_BMCONTROLS,
        &ControlType::BmControl2,
        indent + 2,
    );
    dump_value_string(
        selector.clock_selector_index,
        "iClockSelector",
        selector.clock_selector.as_ref().unwrap_or(&"".into()),
        indent,
        width,
    );
}

/// Dumps the contents of a UAC3 Clock Selector Descriptor
fn dump_audio_clock_selector3(selector: &ClockSelector3, indent: usize, width: usize) {
    dump_value(selector.clock_id, "bClockID", indent, width);
    dump_value(selector.nr_in_pins, "bNrInPins", indent, width);
    dump_array(&selector.csource_ids, "baCSourceID", indent, width);
    dump_hex(selector.controls, "bmControls", indent, width);
    dump_bmcontrols(
        selector.controls,
        &UAC2_CLOCK_SELECTOR_BMCONTROLS,
        &ControlType::BmControl2,
        indent + 2,
    );
    dump_value(
        selector.cselector_descr_str,
        "wCSelectorDescrStr",
        indent,
        width,
    );
}

/// Dumps the contents of a UAC2 Clock Multiplier Descriptor
fn dump_audio_clock_multiplier2(multiplier: &ClockMultiplier2, indent: usize, width: usize) {
    dump_value(multiplier.clock_id, "bClockID", indent, width);
    dump_value(multiplier.csource_id, "bCSourceID", indent, width);
    dump_hex(multiplier.controls, "bmControls", indent, width);
    dump_bmcontrols(
        multiplier.controls,
        &UAC2_CLOCK_MULTIPLIER_BMCONTROLS,
        &ControlType::BmControl2,
        indent + 2,
    );
    dump_value_string(
        multiplier.clock_multiplier_index,
        "iClockMultiplier",
        multiplier.clock_multiplier.as_ref().unwrap_or(&"".into()),
        indent,
        width,
    );
}

/// Dumps the contents of a UAC3 Clock Multiplier Descriptor
fn dump_audio_clock_multiplier3(multiplier: &ClockMultiplier3, indent: usize, width: usize) {
    dump_value(multiplier.clock_id, "bClockID", indent, width);
    dump_value(multiplier.csource_id, "bCSourceID", indent, width);
    dump_hex(multiplier.controls, "bmControls", indent, width);
    dump_bmcontrols(
        multiplier.controls,
        &UAC2_CLOCK_MULTIPLIER_BMCONTROLS,
        &ControlType::BmControl2,
        indent + 2,
    );
    dump_value(
        multiplier.cmultiplier_descr_str,
        "wCMultiplierDescrStr",
        indent,
        width,
    );
}

fn dump_audio_sample_rate_converter2(
    converter: &SampleRateConverter2,
    indent: usize,
    width: usize,
) {
    dump_value(converter.unit_id, "bUnitID", indent, width);
    dump_value(converter.source_id, "bSourceID", indent, width);
    dump_value(converter.csource_in_id, "bCSourceInID", indent, width);
    dump_value(converter.csource_out_id, "bCSourceOutID", indent, width);
    dump_value_string(
        converter.src_index,
        "iSRC",
        converter.src.as_ref().unwrap_or(&"".into()),
        indent,
        width,
    );
}

fn dump_audio_sample_rate_converter3(
    converter: &SampleRateConverter3,
    indent: usize,
    width: usize,
) {
    dump_value(converter.unit_id, "bUnitID", indent, width);
    dump_value(converter.source_id, "bSourceID", indent, width);
    dump_value(converter.csource_in_id, "bCSourceInID", indent, width);
    dump_value(converter.csource_out_id, "bCSourceOutID", indent, width);
    dump_value(converter.src_descr_str, "wSRCDescrStr", indent, width);
}

fn dump_audio_header1(header: &Header1, indent: usize, width: usize) {
    dump_value(header.version, "bcdADC", indent, width);
    dump_value(header.total_length, "wTotalLength", indent, width);
    dump_value(header.collection_bytes, "bInCollection", indent, width);
    dump_array(&header.interfaces, "baInterfaceNr", indent, width);
}

fn dump_audio_header2(header: &Header2, indent: usize, width: usize) {
    dump_value(header.version, "bcdADC", indent, width);
    dump_value(header.total_length, "wTotalLength", indent, width);
    dump_hex(header.controls, "bmControls", indent, width);
    dump_bmcontrols(
        header.controls as u32,
        &UAC2_INTERFACE_HEADER_BMCONTROLS,
        &ControlType::BmControl2,
        indent + 2,
    );
}

fn dump_audio_header3(header: &Header3, indent: usize, width: usize) {
    dump_value(header.category, "bCategory", indent, width);
    dump_value(header.total_length, "wTotalLength", indent, width);
    dump_hex(header.controls, "bmControls", indent, width);
    dump_bmcontrols(
        header.controls,
        &UAC2_INTERFACE_HEADER_BMCONTROLS,
        &ControlType::BmControl2,
        indent + 2,
    );
}

fn dump_audio_input_terminal1(ait: &InputTerminal1, indent: usize, width: usize) {
    dump_value(ait.terminal_id, "bTerminalID", indent, width);
    println!(
        "{:indent$}wTerminalType      {:5} {}",
        "",
        ait.terminal_type,
        names::videoterminal(ait.terminal_type).unwrap_or_default(),
        indent = indent
    );
    dump_value(ait.assoc_terminal, "bAssocTerminal", indent, width);
    dump_value(ait.nr_channels, "bNrChannels", indent, width);
    dump_hex(ait.channel_config, "wChannelConfig", indent, width);
    let channel_names = UacInterfaceDescriptor::get_channel_name_strings(
        &UacProtocol::Uac1,
        ait.channel_config as u32,
    );
    for name in channel_names.iter() {
        println!("{:indent$}{}", "", name, indent = indent + 2);
    }
    dump_value_string(
        ait.channel_names_index,
        "iChannelNames",
        ait.channel_names.as_ref().unwrap_or(&"".into()),
        indent,
        width,
    );
    dump_value_string(
        ait.terminal_index,
        "iTerminal",
        ait.terminal.as_ref().unwrap_or(&"".into()),
        indent,
        width,
    );
}

fn dump_audio_input_terminal2(ait: &InputTerminal2, indent: usize, width: usize) {
    dump_value(ait.terminal_id, "bTerminalID", indent, width);
    dump_name(
        ait.terminal_type,
        names::videoterminal,
        "wTerminalType",
        indent,
        width,
    );
    dump_value(ait.assoc_terminal, "bAssocTerminal", indent, width);
    dump_value(ait.nr_channels, "bNrChannels", indent, width);
    dump_hex(ait.channel_config, "wChannelConfig", indent, width);
    let channel_names =
        UacInterfaceDescriptor::get_channel_name_strings(&UacProtocol::Uac2, ait.channel_config);
    for name in channel_names.iter() {
        println!("{:indent$}{}", "", name, indent = indent + 2);
    }
    dump_value_string(
        ait.channel_names_index,
        "iChannelNames",
        ait.channel_names.as_ref().unwrap_or(&"".into()),
        indent,
        width,
    );
    dump_hex(ait.controls, "bmControls", indent, width);
    dump_bmcontrols(
        ait.controls,
        &UAC2_INPUT_TERMINAL_BMCONTROLS,
        &ControlType::BmControl2,
        indent + 2,
    );
    dump_value(ait.terminal_index, "iTerminal", indent, width);
    dump_value_string(
        ait.terminal_index,
        "iTerminal",
        ait.terminal.as_ref().unwrap_or(&"".into()),
        indent,
        width,
    );
}

fn dump_audio_input_terminal3(ait: &InputTerminal3, indent: usize, width: usize) {
    dump_value(ait.terminal_id, "bTerminalID", indent, width);
    dump_name(
        ait.terminal_type,
        names::videoterminal,
        "wTerminalType",
        indent,
        width,
    );
    dump_value(ait.assoc_terminal, "bAssocTerminal", indent, width);
    dump_value(ait.csource_id, "bCSourceID", indent, width);
    dump_hex(ait.controls, "bmControls", indent, width);
    dump_bmcontrols(
        ait.controls,
        &UAC3_INPUT_TERMINAL_BMCONTROLS,
        &ControlType::BmControl2,
        indent + 2,
    );
    dump_value(ait.cluster_descr_id, "wClusterDescrID", indent, width);
    dump_value(
        ait.ex_terminal_descr_id,
        "wExTerminalDescrID",
        indent,
        width,
    );
    dump_value(ait.connectors_descr_id, "wConnectorDescrId", indent, width);
    dump_value(ait.terminal_descr_str, "wTerminalDescrStr", indent, width);
}

fn dump_audio_output_terminal1(a: &OutputTerminal1, indent: usize, width: usize) {
    dump_value(a.terminal_id, "bTerminalID", indent, width);
    dump_name(
        a.terminal_type,
        names::videoterminal,
        "wTerminalType",
        indent,
        width,
    );
    dump_value(a.assoc_terminal, "bAssocTerminal", indent, width);
    dump_value(a.source_id, "bSourceID", indent, width);
    dump_value_string(
        a.terminal_index,
        "iTerminal",
        a.terminal.as_ref().unwrap_or(&"".into()),
        indent,
        width,
    );
}

fn dump_audio_output_terminal2(a: &OutputTerminal2, indent: usize, width: usize) {
    dump_value(a.terminal_id, "bTerminalID", indent, width);
    dump_name(
        a.terminal_type,
        names::videoterminal,
        "wTerminalType",
        indent,
        width,
    );
    dump_value(a.assoc_terminal, "bAssocTerminal", indent, width);
    dump_value(a.source_id, "bSourceID", indent, width);
    dump_hex(a.controls, "bmControls", indent, width);
    dump_bmcontrols(
        a.controls,
        &UAC2_OUTPUT_TERMINAL_BMCONTROLS,
        &ControlType::BmControl2,
        indent + 2,
    );
    dump_value_string(
        a.terminal_index,
        "iTerminal",
        a.terminal.as_ref().unwrap_or(&"".into()),
        indent,
        width,
    );
}

fn dump_audio_output_terminal3(a: &OutputTerminal3, indent: usize, width: usize) {
    dump_value(a.terminal_id, "bTerminalID", indent, width);
    dump_name(
        a.terminal_type,
        names::videoterminal,
        "wTerminalType",
        indent,
        width,
    );
    dump_value(a.assoc_terminal, "bAssocTerminal", indent, width);
    dump_value(a.c_source_id, "bCSourceID", indent, width);
    dump_hex(a.controls, "bmControls", indent, width);
    dump_bmcontrols(
        a.controls,
        &UAC3_OUTPUT_TERMINAL_BMCONTROLS,
        &ControlType::BmControl2,
        indent + 2,
    );
    dump_value(a.ex_terminal_descr_id, "wExTerminalDescrID", indent, width);
    dump_value(a.connectors_descr_id, "wConnectorDescrId", indent, width);
    dump_value(a.terminal_descr_str, "wTerminalDescrStr", indent, width);
}

fn dump_extended_terminal_header(d: &ExtendedTerminalHeader, indent: usize, width: usize) {
    dump_value(d.descriptor_id, "wDescriptorID", indent, width);
    dump_value(d.nr_channels, "bNrChannels", indent, width);
}

fn dump_audio_streaming_interface1(asi: &StreamingInterface1, indent: usize, width: usize) {
    dump_value(asi.terminal_link, "bTerminalLink", indent, width);
    dump_value(asi.delay, "bDelay", indent, width);
    dump_value(asi.format_tag, "wFormatTag", indent, width);
}

fn dump_audio_streaming_interface2(asi: &StreamingInterface2, indent: usize, width: usize) {
    dump_value(asi.terminal_link, "bTerminalLink", indent, width);
    dump_hex(asi.controls, "bmControls", indent, width);
    dump_bmcontrols(
        asi.controls,
        &UAC2_AS_INTERFACE_BMCONTROLS,
        &ControlType::BmControl2,
        indent + 2,
    );
    dump_value(asi.format_type, "bFormatType", indent, width);
    dump_value(asi.nr_channels, "bNrChannels", indent, width);
    dump_hex(asi.channel_config, "bmChannelConfig", indent, width);
    let channel_names =
        UacInterfaceDescriptor::get_channel_name_strings(&UacProtocol::Uac2, asi.channel_config);
    for name in channel_names.iter() {
        println!("{:indent$}{}", "", name, indent = indent + 2);
    }
    dump_value_string(
        asi.channel_names_index,
        "iChannelNames",
        asi.channel_names.as_ref().unwrap_or(&"".into()),
        indent,
        width,
    );
}

fn dump_audio_streaming_interface3(asi: &StreamingInterface3, indent: usize, width: usize) {
    dump_value(asi.terminal_link, "bTerminalLink", indent, width);
    dump_hex(asi.controls, "bmControls", indent, width);
    dump_bmcontrols(
        asi.controls,
        &UAC3_AS_INTERFACE_BMCONTROLS,
        &ControlType::BmControl2,
        indent + 2,
    );
    dump_value(asi.cluster_descr_id, "wClusterDescrID", indent, width);
    dump_hex(asi.formats, "bmFormats", indent, width);
    dump_value(asi.sub_slot_size, "bSubslotSize", indent, width);
    dump_value(asi.bit_resolution, "bBitResolution", indent, width);
    dump_hex(asi.aux_protocols, "bmAuxProtocols", indent, width);
    dump_value(asi.control_size, "bControlSize", indent, width);
}

fn dump_audio_data_streaming_endpoint1(ads: &DataStreamingEndpoint1, indent: usize, width: usize) {
    let uac1_attrs = |a: usize| match a {
        0x00 => Some("Sampling Frequency"),
        0x01 => Some("Pitch"),
        0x02 => Some("Audio Data Format Control"),
        0x07 => Some("MaxPacketsOnly"),
        _ => None,
    };
    dump_hex(ads.attributes, "bmAttributes", indent, width);
    dump_bitmap_strings(ads.attributes, uac1_attrs, indent + 2);
    dump_value(ads.lock_delay_units, "bLockDelayUnits", indent, width);
    dump_value(ads.lock_delay, "wLockDelay", indent, width);
}

fn dump_audio_data_streaming_endpoint2(ads: &DataStreamingEndpoint2, indent: usize, width: usize) {
    let uac2_attrs = |attr: usize| match attr {
        0x07 => Some("MaxPacketsOnly"),
        _ => None,
    };
    dump_hex(ads.attributes, "bmAttributes", indent, width);
    dump_bitmap_strings(ads.attributes, uac2_attrs, indent + 2);
    dump_hex(ads.controls, "bmControls", indent, width);
    dump_bmcontrols(
        ads.controls,
        &UAC2_AS_ISO_ENDPOINT_BMCONTROLS,
        &ControlType::BmControl2,
        indent + 2,
    );
    dump_value(ads.lock_delay_units, "bLockDelayUnits", indent, width);
    dump_value(ads.lock_delay, "wLockDelay", indent, width);
}

fn dump_audio_data_streaming_endpoint3(ads: &DataStreamingEndpoint3, indent: usize, width: usize) {
    dump_hex(ads.controls, "bmControls", indent, width);
    dump_bmcontrols(
        ads.controls,
        &UAC2_AS_ISO_ENDPOINT_BMCONTROLS,
        &ControlType::BmControl2,
        indent + 2,
    );
    dump_value(ads.lock_delay_units, "bLockDelayUnits", indent, width);
    dump_value(ads.lock_delay, "wLockDelay", indent, width);
}

fn dump_audio_subtype(uacid: &UacInterfaceDescriptor, indent: usize) {
    match uacid {
        UacInterfaceDescriptor::Header1(a) => {
            dump_audio_header1(a, indent, LSUSB_DUMP_WIDTH);
        }
        UacInterfaceDescriptor::Header2(ach) => {
            dump_audio_header2(ach, indent, LSUSB_DUMP_WIDTH);
        }
        UacInterfaceDescriptor::Header3(ach) => {
            dump_audio_header3(ach, indent, LSUSB_DUMP_WIDTH);
        }
        UacInterfaceDescriptor::InputTerminal1(ait) => {
            dump_audio_input_terminal1(ait, indent, LSUSB_DUMP_WIDTH);
        }
        UacInterfaceDescriptor::InputTerminal2(ait) => {
            dump_audio_input_terminal2(ait, indent, LSUSB_DUMP_WIDTH);
        }
        UacInterfaceDescriptor::InputTerminal3(ait) => {
            dump_audio_input_terminal3(ait, indent, LSUSB_DUMP_WIDTH);
        }
        UacInterfaceDescriptor::OutputTerminal1(a) => {
            dump_audio_output_terminal1(a, indent, LSUSB_DUMP_WIDTH);
        }
        UacInterfaceDescriptor::OutputTerminal2(a) => {
            dump_audio_output_terminal2(a, indent, LSUSB_DUMP_WIDTH);
        }
        UacInterfaceDescriptor::OutputTerminal3(a) => {
            dump_audio_output_terminal3(a, indent, LSUSB_DUMP_WIDTH);
        }
        UacInterfaceDescriptor::ExtendedTerminalHeader(d) => {
            dump_extended_terminal_header(d, indent, LSUSB_DUMP_WIDTH);
        }
        UacInterfaceDescriptor::PowerDomain(power_domain) => {
            dump_audio_power_domain(power_domain, indent, LSUSB_DUMP_WIDTH);
        }
        UacInterfaceDescriptor::MixerUnit1(mixer_unit) => {
            dump_audio_mixer_unit1(mixer_unit, indent, LSUSB_DUMP_WIDTH);
        }
        UacInterfaceDescriptor::MixerUnit2(mixer_unit) => {
            dump_audio_mixer_unit2(mixer_unit, indent, LSUSB_DUMP_WIDTH);
        }
        UacInterfaceDescriptor::MixerUnit3(mixer_unit) => {
            dump_audio_mixer_unit3(mixer_unit, indent, LSUSB_DUMP_WIDTH);
        }
        UacInterfaceDescriptor::SelectorUnit1(selector_unit) => {
            dump_audio_selector_unit1(selector_unit, indent, LSUSB_DUMP_WIDTH);
        }
        UacInterfaceDescriptor::SelectorUnit2(selector_unit) => {
            dump_audio_selector_unit2(selector_unit, indent, LSUSB_DUMP_WIDTH);
        }
        UacInterfaceDescriptor::SelectorUnit3(selector_unit) => {
            dump_audio_selector_unit3(selector_unit, indent, LSUSB_DUMP_WIDTH);
        }
        UacInterfaceDescriptor::ProcessingUnit1(unit) => {
            dump_audio_processing_unit1(unit, indent, LSUSB_DUMP_WIDTH);
        }
        UacInterfaceDescriptor::ProcessingUnit2(unit) => {
            dump_audio_processing_unit2(unit, indent, LSUSB_DUMP_WIDTH);
        }
        UacInterfaceDescriptor::ProcessingUnit3(unit) => {
            dump_audio_processing_unit3(unit, indent, LSUSB_DUMP_WIDTH);
        }
        UacInterfaceDescriptor::EffectUnit2(unit) => {
            dump_audio_effect_unit2(unit, indent, LSUSB_DUMP_WIDTH);
        }
        UacInterfaceDescriptor::EffectUnit3(unit) => {
            dump_audio_effect_unit3(unit, indent, LSUSB_DUMP_WIDTH);
        }
        UacInterfaceDescriptor::FeatureUnit1(unit) => {
            dump_audio_feature_unit1(unit, indent, LSUSB_DUMP_WIDTH);
        }
        UacInterfaceDescriptor::FeatureUnit2(unit) => {
            dump_audio_feature_unit2(unit, indent, LSUSB_DUMP_WIDTH);
        }
        UacInterfaceDescriptor::FeatureUnit3(unit) => {
            dump_audio_feature_unit3(unit, indent, LSUSB_DUMP_WIDTH);
        }
        UacInterfaceDescriptor::ExtensionUnit1(unit) => {
            dump_audio_extension_unit1(unit, indent, LSUSB_DUMP_WIDTH);
        }
        UacInterfaceDescriptor::ExtensionUnit2(unit) => {
            dump_audio_extension_unit2(unit, indent, LSUSB_DUMP_WIDTH);
        }
        UacInterfaceDescriptor::ExtensionUnit3(unit) => {
            dump_audio_extension_unit3(unit, indent, LSUSB_DUMP_WIDTH);
        }
        UacInterfaceDescriptor::ClockSource2(source) => {
            dump_audio_clock_source2(source, indent, LSUSB_DUMP_WIDTH);
        }
        UacInterfaceDescriptor::ClockSource3(source) => {
            dump_audio_clock_source3(source, indent, LSUSB_DUMP_WIDTH);
        }
        UacInterfaceDescriptor::ClockSelector2(selector) => {
            dump_audio_clock_selector2(selector, indent, LSUSB_DUMP_WIDTH);
        }
        UacInterfaceDescriptor::ClockSelector3(selector) => {
            dump_audio_clock_selector3(selector, indent, LSUSB_DUMP_WIDTH);
        }
        UacInterfaceDescriptor::ClockMultiplier2(multiplier) => {
            dump_audio_clock_multiplier2(multiplier, indent, LSUSB_DUMP_WIDTH);
        }
        UacInterfaceDescriptor::ClockMultiplier3(multiplier) => {
            dump_audio_clock_multiplier3(multiplier, indent, LSUSB_DUMP_WIDTH);
        }
        UacInterfaceDescriptor::SampleRateConverter2(converter) => {
            dump_audio_sample_rate_converter2(converter, indent, LSUSB_DUMP_WIDTH);
        }
        UacInterfaceDescriptor::SampleRateConverter3(converter) => {
            dump_audio_sample_rate_converter3(converter, indent, LSUSB_DUMP_WIDTH);
        }
        UacInterfaceDescriptor::StreamingInterface1(asi) => {
            dump_audio_streaming_interface1(asi, indent, LSUSB_DUMP_WIDTH);
        }
        UacInterfaceDescriptor::StreamingInterface2(asi) => {
            dump_audio_streaming_interface2(asi, indent, LSUSB_DUMP_WIDTH);
        }
        UacInterfaceDescriptor::StreamingInterface3(asi) => {
            dump_audio_streaming_interface3(asi, indent, LSUSB_DUMP_WIDTH);
        }
        UacInterfaceDescriptor::DataStreamingEndpoint1(ads) => {
            dump_audio_data_streaming_endpoint1(ads, indent, LSUSB_DUMP_WIDTH);
        }
        UacInterfaceDescriptor::DatastreamingEndpoint2(ads) => {
            dump_audio_data_streaming_endpoint2(ads, indent, LSUSB_DUMP_WIDTH);
        }
        UacInterfaceDescriptor::DataStreamingEndpoint3(ads) => {
            dump_audio_data_streaming_endpoint3(ads, indent, LSUSB_DUMP_WIDTH);
        }
        UacInterfaceDescriptor::Undefined(data) => {
            println!(
                "{:indent$}Invalid desc subtype: {}",
                "",
                data.iter()
                    .map(|b| format!("{:02x}", b))
                    .collect::<Vec<String>>()
                    .join(" "),
                indent = indent * 2
            );
        }
        _ => (),
    }
}

fn dump_audiocontrol_interface(
    uacd: &UacDescriptor,
    uaci: &ControlSubtype,
    protocol: &UacProtocol,
) {
    dump_title("AudioControl Interface Descriptor", 6);
    dump_value(uacd.length, "bLength", 8, LSUSB_DUMP_WIDTH);
    dump_value(uacd.descriptor_type, "bDescriptorType", 8, LSUSB_DUMP_WIDTH);
    dump_value_string(
        uaci.to_owned() as u8,
        "bDescriptorSubtype",
        format!("({:#})", uaci),
        8,
        LSUSB_DUMP_WIDTH,
    );

    match &uacd.interface {
        UacInterfaceDescriptor::Invalid(_) => {
            println!(
                "{:indent$}Warning: {:#} descriptors are illegal for {}",
                "",
                uacd.subtype,
                u8::from(protocol.to_owned()),
                indent = 6
            );
        }
        uacid => dump_audio_subtype(uacid, 8),
    }
}

fn dump_audiostreaming_interface(
    uacd: &UacDescriptor,
    uasi: &StreamingSubtype,
    protocol: &UacProtocol,
) {
    println!("      AudioStreaming Interface Descriptor:");
    println!("        bLength              {:3}", uacd.length);
    println!("        bDescriptorType      {:3}", uacd.descriptor_type);
    print!("        bDescriptorSubtype   {:3} ", uasi.to_owned() as u8);

    match uasi {
        StreamingSubtype::General | StreamingSubtype::Undefined => {
            println!("({:#})", uacd.subtype);
            match &uacd.interface {
                UacInterfaceDescriptor::Invalid(_) => {
                    println!(
                        "{:indent$}Warning: {:#} descriptors are illegal for {}",
                        "",
                        uacd.subtype,
                        u8::from(protocol.to_owned()),
                        indent = 6
                    );
                }
                uacid => dump_audio_subtype(uacid, 8),
            }
        }
        StreamingSubtype::FormatType => {
            println!("(FORMAT_TYPE)");
            let data: Vec<u8> = uacd.interface.to_owned().into();
            match protocol {
                UacProtocol::Uac1 => {
                    if data.len() < 5 {
                        println!("      Warning: Descriptor too short");
                        return;
                    }
                    print!("        bFormatType        {:5} ", data[0]);
                    match data[0] {
                        0x01 => dump_format_type_i(&data),
                        0x02 => dump_format_type_ii(&data),
                        0x03 => dump_format_type_iii(&data),
                        _ => println!(
                            "(invalid)\n        Invalid desc format type: {}",
                            data[1..]
                                .iter()
                                .map(|b| format!("{:02x}", b))
                                .collect::<Vec<String>>()
                                .join("")
                        ),
                    }
                }
                UacProtocol::Uac2 => {
                    if data.is_empty() {
                        println!("      Warning: Descriptor too short");
                        return;
                    }
                    print!("        bFormatType        {:5} ", data[0]);
                    match data[0] {
                        0x01 => dump_format_type_i_uac2(&data),
                        0x02 => dump_format_type_ii_uac2(&data),
                        0x03 => dump_format_type_iii_uac2(&data),
                        0x04 => dump_format_type_iv_uac2(&data),
                        _ => println!(
                            "(invalid)\n        Invalid desc format type: {}",
                            data[1..]
                                .iter()
                                .map(|b| format!("{:02x}", b))
                                .collect::<Vec<String>>()
                                .join("")
                        ),
                    }
                }
                _ => println!(
                    "(unknown)\n        Invalid desc format type: {}",
                    data[1..]
                        .iter()
                        .map(|b| format!("{:02x}", b))
                        .collect::<Vec<String>>()
                        .join("")
                ),
            }
        }
        StreamingSubtype::FormatSpecific => {
            let data: Vec<u8> = uacd.interface.to_owned().into();
            println!("(FORMAT_SPECIFIC)");
            if data.len() < 2 {
                println!("      Warning: Descriptor too short");
                return;
            }
            let fmttag = u16::from_le_bytes([data[0], data[1]]);
            let fmtptr = get_format_specific_string(fmttag);
            println!("        wFormatTag          {:5} {}", fmttag, fmtptr);
            match fmttag {
                0x1001 => dump_format_specific_mpeg(&data),
                0x1002 => dump_format_specific_ac3(&data),
                _ => println!(
                    "        Invalid desc format type: {}",
                    data[2..]
                        .iter()
                        .map(|b| format!("{:02x}", b))
                        .collect::<Vec<String>>()
                        .join("")
                ),
            }
        }
    }
}

fn get_format_specific_string(fmttag: u16) -> &'static str {
    const FMT_ITAG: [&str; 6] = [
        "TYPE_I_UNDEFINED",
        "PCM",
        "PCM8",
        "IEEE_FLOAT",
        "ALAW",
        "MULAW",
    ];
    const FMT_IITAG: [&str; 3] = ["TYPE_II_UNDEFINED", "MPEG", "AC-3"];
    const FMT_IIITAG: [&str; 7] = [
        "TYPE_III_UNDEFINED",
        "IEC1937_AC-3",
        "IEC1937_MPEG-1_Layer1",
        "IEC1937_MPEG-Layer2/3/NOEXT",
        "IEC1937_MPEG-2_EXT",
        "IEC1937_MPEG-2_Layer1_LS",
        "IEC1937_MPEG-2_Layer2/3_LS",
    ];

    match fmttag {
        0..=5 => FMT_ITAG[fmttag as usize],
        0x1000..=0x1002 => FMT_IITAG[(fmttag & 0xfff) as usize],
        0x2000..=0x2006 => FMT_IIITAG[(fmttag & 0xfff) as usize],
        _ => "undefined",
    }
}

fn dump_format_type_i(data: &[u8]) {
    println!("(FORMAT_TYPE_I)");
    let len = if data[4] != 0 {
        data[4] as usize * 3 + 5
    } else {
        11
    };
    if data.len() < len {
        println!("      Warning: Descriptor too short");
        return;
    }
    println!("        bNrChannels        {:5}", data[1]);
    println!("        bSubframeSize      {:5}", data[2]);
    println!("        bBitResolution     {:5}", data[3]);
    println!(
        "        bSamFreqType       {:5} {}",
        data[4],
        if data[4] != 0 {
            "Discrete"
        } else {
            "Continuous"
        }
    );
    if data[4] == 0 {
        if data.len() < 11 {
            println!("      Warning: Descriptor too short for continuous sample frequency");
            return;
        }
        println!(
            "        tLowerSamFreq    {:7}",
            u32::from_le_bytes([data[5], data[6], data[7], 0])
        );
        println!(
            "        tUpperSamFreq    {:7}",
            u32::from_le_bytes([data[8], data[9], data[10], 0])
        );
    } else {
        for i in 0..data[4] {
            if data.len() < 5 + 3 * (i as usize + 1) {
                println!("      Warning: Descriptor too short for discrete sample frequency");
                return;
            }
            println!(
                "        tSamFreq[{:2}]   {:7}",
                i,
                u32::from_le_bytes([
                    data[5 + 3 * i as usize],
                    data[6 + 3 * i as usize],
                    data[7 + 3 * i as usize],
                    0
                ])
            );
        }
    }
}

fn dump_format_type_ii(data: &[u8]) {
    println!("(FORMAT_TYPE_II)");
    let len = if data[5] != 0 {
        data[4] as usize * 3 + 6
    } else {
        12
    };
    if data.len() < len {
        println!("      Warning: Descriptor too short");
        return;
    }
    println!(
        "        wMaxBitRate        {:5}",
        u16::from_le_bytes([data[1], data[2]])
    );
    println!(
        "        wSamplesPerFrame   {:5}",
        u16::from_le_bytes([data[3], data[4]])
    );
    println!(
        "        bSamFreqType       {:5} {}",
        data[5],
        if data[5] != 0 {
            "Discrete"
        } else {
            "Continuous"
        }
    );
    if data[5] == 0 {
        if data.len() < 12 {
            println!("      Warning: Descriptor too short for continuous sample frequency");
            return;
        }
        println!(
            "        tLowerSamFreq    {:7}",
            u32::from_le_bytes([data[6], data[7], data[8], 0])
        );
        println!(
            "        tUpperSamFreq    {:7}",
            u32::from_le_bytes([data[9], data[10], data[11], 0])
        );
    } else {
        for i in 0..data[5] {
            if data.len() < 6 + 3 * (i as usize + 1) {
                println!("      Warning: Descriptor too short for discrete sample frequency");
                return;
            }
            println!(
                "        tSamFreq[{:2}]     {:7}",
                i,
                u32::from_le_bytes([
                    data[6 + 3 * i as usize],
                    data[7 + 3 * i as usize],
                    data[8 + 3 * i as usize],
                    0
                ])
            );
        }
    }
}

fn dump_format_type_iii(data: &[u8]) {
    println!("(FORMAT_TYPE_III)");
    let len = if data[4] != 0 {
        data[4] as usize * 3 + 5
    } else {
        11
    };
    if data.len() < len {
        println!("      Warning: Descriptor too short");
        return;
    }
    println!("        bNrChannels        {:5}", data[1]);
    println!("        bSubframeSize      {:5}", data[2]);
    println!("        bBitResolution     {:5}", data[3]);
    println!(
        "        bSamFreqType       {:5} {}",
        data[4],
        if data[4] != 0 {
            "Discrete"
        } else {
            "Continuous"
        }
    );
    if data[4] == 0 {
        if data.len() < 11 {
            println!("      Warning: Descriptor too short for continuous sample frequency");
            return;
        }
        println!(
            "        tLowerSamFreq    {:7}",
            u32::from_le_bytes([data[5], data[6], data[7], 0])
        );
        println!(
            "        tUpperSamFreq    {:7}",
            u32::from_le_bytes([data[8], data[9], data[10], 0])
        );
    } else {
        for i in 0..data[4] {
            if data.len() < 5 + 3 * (i as usize + 1) {
                println!("      Warning: Descriptor too short for discrete sample frequency");
                return;
            }
            println!(
                "        tSamFreq[{:2}]   {:7}",
                i,
                u32::from_le_bytes([
                    data[5 + 3 * i as usize],
                    data[6 + 3 * i as usize],
                    data[7 + 3 * i as usize],
                    0
                ])
            );
        }
    }
}

fn dump_format_type_i_uac2(data: &[u8]) {
    println!("(FORMAT_TYPE_I)");
    if data.len() < 3 {
        println!("      Warning: Descriptor too short");
        return;
    }
    println!("        bSubslotSize       {:5}", data[1]);
    println!("        bBitResolution     {:5}", data[2]);
}

fn dump_format_type_ii_uac2(data: &[u8]) {
    println!("(FORMAT_TYPE_II)");
    if data.len() < 5 {
        println!("      Warning: Descriptor too short");
        return;
    }
    println!(
        "        wMaxBitRate        {:5}",
        u16::from_le_bytes([data[1], data[2]])
    );
    println!(
        "        wSlotsPerFrame     {:5}",
        u16::from_le_bytes([data[3], data[4]])
    );
}

fn dump_format_type_iii_uac2(data: &[u8]) {
    println!("(FORMAT_TYPE_III)");
    if data.len() < 3 {
        println!("      Warning: Descriptor too short");
        return;
    }
    println!("        bSubslotSize       {:5}", data[1]);
    println!("        bBitResolution     {:5}", data[2]);
}

fn dump_format_type_iv_uac2(data: &[u8]) {
    println!("(FORMAT_TYPE_IV)");
    if data.is_empty() {
        println!("      Warning: Descriptor too short");
        return;
    }
    println!("        bFormatType        {:5}", data[0]);
}

fn dump_format_specific_mpeg(data: &[u8]) {
    if data.len() < 5 {
        println!("      Warning: Descriptor too short");
        return;
    }
    println!(
        "        bmMPEGCapabilities 0x{:04x}",
        u16::from_le_bytes([data[2], data[3]])
    );
    if data[2] & 0x01 != 0 {
        println!("          Layer I");
    }
    if data[2] & 0x02 != 0 {
        println!("          Layer II");
    }
    if data[2] & 0x04 != 0 {
        println!("          Layer III");
    }
    if data[2] & 0x08 != 0 {
        println!("          MPEG-1 only");
    }
    if data[2] & 0x10 != 0 {
        println!("          MPEG-1 dual-channel");
    }
    if data[2] & 0x20 != 0 {
        println!("          MPEG-2 second stereo");
    }
    if data[2] & 0x40 != 0 {
        println!("          MPEG-2 7.1 channel augmentation");
    }
    if data[2] & 0x80 != 0 {
        println!("          Adaptive multi-channel prediction");
    }
    println!(
        "          MPEG-2 multilingual support: {}",
        match data[3] & 3 {
            0 => "Not supported",
            1 => "Supported at Fs",
            2 => "Reserved",
            _ => "Supported at Fs and 1/2Fs",
        }
    );
    println!("        bmMPEGFeatures       0x{:02x}", data[4]);
    println!(
        "          Internal Dynamic Range Control: {}",
        match (data[4] >> 4) & 3 {
            0 => "not supported",
            1 => "supported but not scalable",
            2 => "scalable, common boost and cut scaling value",
            _ => "scalable, separate boost and cut scaling value",
        }
    );
}

fn dump_format_specific_ac3(data: &[u8]) {
    if data.len() < 7 {
        println!("      Warning: Descriptor too short");
        return;
    }
    println!(
        "        bmBSID         0x{:08x}",
        u32::from_le_bytes([data[2], data[3], data[4], data[5]])
    );
    println!("        bmAC3Features        0x{:02x}", data[6]);
    if data[6] & 0x01 != 0 {
        println!("          RF mode");
    }
    if data[6] & 0x02 != 0 {
        println!("          Line mode");
    }
    if data[6] & 0x04 != 0 {
        println!("          Custom0 mode");
    }
    if data[6] & 0x08 != 0 {
        println!("          Custom1 mode");
    }
    println!(
        "          Internal Dynamic Range Control: {}",
        match (data[6] >> 4) & 3 {
            0 => "not supported",
            1 => "supported but not scalable",
            2 => "scalable, common boost and cut scaling value",
            _ => "scalable, separate boost and cut scaling value",
        }
    );
}

fn dump_midistreaming_interface(md: &MidiDescriptor) {
    let jack_types = |t: u8| match t {
        0x00 => "Undefined",
        0x01 => "Embedded",
        0x02 => "External",
        _ => "Invalid",
    };

    println!("      MIDIStreaming Interface Descriptor:");
    println!("        bLength              {:5}", md.length);
    println!("        bDescriptorType      {:5}", md.descriptor_type);
    print!(
        "      bDescriptorSubtype   {:5} ",
        md.midi_type.to_owned() as u8
    );

    match md.midi_type {
        MidiSubtype::Header => {
            println!("(HEADER)");
            if md.data.len() >= 4 {
                let total_length = u16::from_le_bytes([md.data[2], md.data[3]]);
                println!(
                    "        bcdADC              {:2x}.{:02x}",
                    md.data[1], md.data[0]
                );
                println!("        wTotalLength       0x{:04x}", total_length);
            }
            dump_junk(&md.data, 8, md.length as usize - 3, 4);
        }
        MidiSubtype::InputJack => {
            println!("(MIDI_IN_JACK)");
            if md.data.len() >= 3 {
                println!(
                    "        bJackType           {:5} {}",
                    md.data[0],
                    jack_types(md.data[0])
                );
                println!("        bJackID             {:5}", md.data[1]);
                println!(
                    "        iJack               {:5} {}",
                    md.data[2],
                    md.string.as_ref().unwrap_or(&String::new())
                );
            }
            dump_junk(&md.data, 8, md.length as usize - 3, 4);
        }
        MidiSubtype::OutputJack => {
            println!("(MIDI_OUT_JACK)");
            if md.data.len() >= md.length as usize - 3 {
                println!(
                    "        bJackType           {:5} {}",
                    md.data[0],
                    jack_types(md.data[0])
                );
                println!("        bJackID             {:5}", md.data[1]);
                println!("        bNrInputPins        {:5}", md.data[2]);

                for (i, b) in md.data[3..].chunks(2).enumerate() {
                    if i == md.data[2] as usize {
                        break;
                    }
                    println!("        baSourceID({:2})     {:5}", i, b[0]);
                    println!("        baSourcePin({:2})    {:5}", i, b[1]);
                }

                println!(
                    "        iJack               {:5} {}",
                    md.data[3 + md.data[2] as usize],
                    md.string.as_ref().unwrap_or(&String::new())
                );
                dump_junk(&md.data, 8, md.length as usize - 3, 4 + md.data[2] as usize);
            }
        }
        MidiSubtype::Element => {
            println!("(ELEMENT)");
            if md.data.len() >= md.length as usize - 3 {
                let num_inputs = md.data[1] as usize;
                println!("        bElementID          {:5}", md.data[0]);
                println!("        bNrInputPins        {:5}", num_inputs);
                for (i, b) in md.data[2..].chunks(2).enumerate() {
                    if i == num_inputs {
                        break;
                    }
                    println!("        baSourceID({:2})     {:5}", i, b[0]);
                    println!("        baSourcePin({:2})    {:5}", i, b[1]);
                }
                let j = 2 + num_inputs * 2;
                println!("        bNrOutputPins       {:5}", md.data[j]);
                println!("        bInTerminalLink     {:5}", md.data[j + 1]);
                println!("        bOutTerminalLink    {:5}", md.data[j + 2]);
                println!("        bElCapsSize         {:5}", md.data[j + 3]);
                let capsize = md.data[j + 3] as usize;
                let mut caps: u16 = 0;
                for j in 0..capsize {
                    caps |= (md.data[j + 6 + num_inputs * 2] as u16) << (j * 8);
                }
                println!("        bmElementCaps  0x{:08x}", caps);
                if caps & 0x01 != 0 {
                    println!("          Undefined");
                }
                if caps & 0x02 != 0 {
                    println!("          MIDI Clock");
                }
                if caps & 0x04 != 0 {
                    println!("          MTC (MIDI Time Code)");
                }
                if caps & 0x08 != 0 {
                    println!("          MMC (MIDI Machine Control)");
                }
                if caps & 0x10 != 0 {
                    println!("          GM1 (General MIDI v.1)");
                }
                if caps & 0x20 != 0 {
                    println!("          GM2 (General MIDI v.2)");
                }
                if caps & 0x40 != 0 {
                    println!("          GS MIDI Extension");
                }
                if caps & 0x80 != 0 {
                    println!("          XG MIDI Extension");
                }
                if caps & 0x0100 != 0 {
                    println!("          EFX");
                }
                if caps & 0x0200 != 0 {
                    println!("          MIDI Patch Bay");
                }
                if caps & 0x0400 != 0 {
                    println!("          DLS1 (Downloadable Sounds Level 1)");
                }
                if caps & 0x0800 != 0 {
                    println!("          DLS2 (Downloadable Sounds Level 2)");
                }

                println!(
                    "        iElement            {:5} {}",
                    md.data[2 + md.data[1] as usize],
                    md.string.as_ref().unwrap_or(&String::new())
                );
                dump_junk(&md.data, 8, md.length as usize - 3, j + 1_usize);
            }
        }
        _ => {
            println!(
                "(invalid)\n        Invalid desc subtype: {}",
                md.data
                    .iter()
                    .map(|b| format!("{:02x}", b))
                    .collect::<Vec<String>>()
                    .join(" ")
            );
        }
    }
}

fn dump_videocontrol_interface(vcd: &UvcDescriptor, protocol: u8) {
    println!("      VideoControl Interface Descriptor:");
    println!("        bLength             {:5}", vcd.length);
    println!("        bDescriptorType     {:5}", vcd.descriptor_type);
    print!(
        "        bDescriptorSubtype  {:5} ",
        (vcd.subtype.to_owned() as u8)
    );

    match vcd.subtype {
        UvcSubtype::Header => {
            println!("(HEADER)");
            if vcd.data.len() >= 10 {
                let n = vcd.data[8] as usize;
                let freq = u32::from_le_bytes([vcd.data[4], vcd.data[5], vcd.data[6], vcd.data[7]]);
                println!(
                    "        bcdUVC              {:2x}.{:02x}",
                    vcd.data[1], vcd.data[0]
                );
                println!(
                    "        wTotalLength       0x{:04x}",
                    u16::from_le_bytes([vcd.data[2], vcd.data[3]])
                );
                println!(
                    "        dwClockFrequency    {:5}.{:06}MHz",
                    freq / 1000000,
                    freq % 1000000
                );
                println!("        bInCollection       {:5}", n);
                for (i, b) in vcd.data[9..].iter().enumerate() {
                    if i == n {
                        break;
                    }
                    println!("        baInterfaceNr({:2})   {:5}", i, b);
                }

                dump_junk(&vcd.data, 8, vcd.length as usize - 3, 9 + n);
            }
        }
        UvcSubtype::InputTerminal => {
            println!("(INPUT_TERMINAL)");
            if vcd.data.len() >= 10 {
                let term_type = u16::from_le_bytes([vcd.data[1], vcd.data[2]]);
                let mut n = if term_type == 0x0201 { 7 } else { 0 };
                println!("        bTerminalID         {:5}", vcd.data[0]);
                println!(
                    "        wTerminalType      0x{:04x} {}",
                    term_type,
                    names::videoterminal(term_type).unwrap_or_default()
                );
                println!("        bAssocTerminal      {:5}", vcd.data[3]);
                println!(
                    "        iTerminal           {:5} {}",
                    vcd.data[4],
                    vcd.string.as_ref().unwrap_or(&String::new())
                );

                if term_type == 0x0201 {
                    n += vcd.data[11] as usize;
                    println!(
                        "        wObjectiveFocalLengthMin  {:5}",
                        u16::from_le_bytes([vcd.data[5], vcd.data[6]])
                    );
                    println!(
                        "        wObjectiveFocalLengthMax  {:5}",
                        u16::from_le_bytes([vcd.data[7], vcd.data[8]])
                    );
                    println!(
                        "        wOcularFocalLength        {:5}",
                        u16::from_le_bytes([vcd.data[9], vcd.data[10]])
                    );
                    println!("        bControlSize              {:5}", vcd.data[11]);

                    let mut controls: u32 = 0;
                    for i in 0..3 {
                        if i < vcd.data[11] as usize {
                            controls = (controls << 8) | vcd.data[5 + n - i - 1] as u32;
                        }
                    }
                    println!("        bmControls           0x{:08x}", controls);

                    if protocol == 0x01 {
                        for (i, n) in CAM_CTRL_NAMES.iter().enumerate().take(22) {
                            if (controls >> i) & 1 != 0 {
                                println!("         {}", n);
                            }
                        }
                    } else {
                        for (i, n) in CAM_CTRL_NAMES.iter().enumerate().take(19) {
                            if (controls >> i) & 1 != 0 {
                                println!("         {}", n);
                            }
                        }
                    }
                }

                dump_junk(&vcd.data, 8, vcd.length as usize - 3, 5 + n);
            } else {
                println!("      Warning: Descriptor too short");
            }
        }
        UvcSubtype::OutputTerminal => {
            println!("(OUTPUT_TERMINAL)");
            if vcd.data.len() >= 6 {
                let term_type = u16::from_le_bytes([vcd.data[1], vcd.data[2]]);
                println!("        bTerminalID         {:5}", vcd.data[0]);
                println!(
                    "        wTerminalType      0x{:04x} {}",
                    term_type,
                    names::videoterminal(term_type).unwrap_or_default()
                );
                println!("        bAssocTerminal      {:5}", vcd.data[3]);
                println!("        bSourceID           {:5}", vcd.data[4]);
                println!(
                    "        iTerminal           {:5} {}",
                    vcd.data[5],
                    vcd.string.as_ref().unwrap_or(&String::new())
                );
            } else {
                println!("      Warning: Descriptor too short");
            }

            dump_junk(&vcd.data, 8, vcd.length as usize - 3, 6);
        }
        UvcSubtype::SelectorUnit => {
            println!("(SELECTOR_UNIT)");
            if vcd.data.len() >= 4 {
                let pins = vcd.data[1] as usize;
                println!("        bUnitID             {:5}", vcd.data[0]);
                println!("        bNrInPins           {:5}", pins);
                for (i, b) in vcd.data[2..].iter().enumerate() {
                    if i == pins {
                        break;
                    }
                    println!("        baSourceID({:2})        {:5}", i, b);
                }
                println!(
                    "        iSelector           {:5} {}",
                    vcd.data[2 + pins],
                    vcd.string.as_ref().unwrap_or(&String::new())
                );

                dump_junk(&vcd.data, 8, vcd.length as usize - 3, 3 + pins);
            } else {
                println!("      Warning: Descriptor too short");
            }
        }
        UvcSubtype::ProcessingUnit => {
            println!("(PROCESSING_UNIT)");
            if vcd.data.len() >= 9 {
                let n = vcd.data[4] as usize;
                println!("        bUnitID             {:5}", vcd.data[0]);
                println!("        bSourceID           {:5}", vcd.data[1]);
                println!(
                    "        wMaxMultiplier      {:5}",
                    u16::from_le_bytes([vcd.data[2], vcd.data[3]])
                );
                println!("        bControlSize        {:5}", n);

                let mut controls: u32 = 0;
                for i in 0..3 {
                    if i < n {
                        controls = (controls << 8) | vcd.data[5 + n - i - 1] as u32;
                    }
                }
                println!("        bmControls     0x{:08x}", controls);
                if protocol == 0x01 {
                    for (i, n) in CTRL_NAMES.iter().enumerate().take(19) {
                        if (controls >> i) & 1 != 0 {
                            println!("         {}", n);
                        }
                    }
                } else {
                    for (i, n) in CTRL_NAMES.iter().enumerate().take(18) {
                        if (controls >> i) & 1 != 0 {
                            println!("         {}", n);
                        }
                    }
                }
                let stds = vcd.data[6 + n] as usize;
                println!(
                    "        iProcessing         {:5} {}",
                    vcd.data[5 + n],
                    vcd.string.as_ref().unwrap_or(&String::new())
                );
                println!("        bmVideoStandards     0x{:02x}", stds);
                for (i, n) in STD_NAMES.iter().enumerate().take(6) {
                    if (stds >> i) & 1 != 0 {
                        println!("         {}", n);
                    }
                }
            } else {
                println!("      Warning: Descriptor too short");
            }
        }
        UvcSubtype::ExtensionUnit => {
            println!("(EXTENSION_UNIT)");
            if vcd.data.len() >= 21 {
                let p = vcd.data[18] as usize;
                let n = vcd.data[19 + p] as usize;
                println!("        bUnitID             {:5}", vcd.data[0]);
                println!(
                    "        guidExtensionCode         {}",
                    get_guid(&vcd.data[1..17])
                );
                println!("        bNumControls        {:5}", vcd.data[17]);
                println!("        bNrInPins           {:5}", vcd.data[18]);

                if vcd.data.len() >= 21 + p + n {
                    for (i, b) in vcd.data[19..19 + p].iter().enumerate() {
                        println!("        baSourceID({:2})      {:5}", i, b);
                    }
                    println!("        bControlSize        {:5}", vcd.data[19 + p]);
                    for (i, b) in vcd.data[20 + p..20 + p + n].iter().enumerate() {
                        println!("        bmControls({:2})       0x{:02x}", i, b);
                    }
                    println!(
                        "        iExtension          {:5} {}",
                        vcd.data[20 + p + n],
                        vcd.string.as_ref().unwrap_or(&String::new())
                    );
                }

                dump_junk(&vcd.data, 8, vcd.length as usize - 3, 21 + p + n);
            } else {
                println!("      Warning: Descriptor too short");
            }
        }
        UvcSubtype::EncodingUnit => {
            println!("(ENCODING_UNIT)");
            if vcd.data.len() >= 10 {
                println!("        bUnitID             {:5}", vcd.data[0]);
                println!("        bSourceID           {:5}", vcd.data[1]);
                println!(
                    "        iEncoding           {:5} {}",
                    vcd.data[2],
                    vcd.string.as_ref().unwrap_or(&String::new())
                );
                println!("        bControlSize        {:5}", vcd.data[3]);

                let mut controls: u32 = 0;
                for i in 0..3 {
                    controls = (controls << 8) | vcd.data[6 - i] as u32;
                }
                println!("        bmControls              0x{:08x}", controls);
                for (i, n) in EN_CTRL_NAMES.iter().enumerate().take(20) {
                    if (controls >> i) & 1 != 0 {
                        println!("         {}", n); // Replace with your Rust lookup approach
                    }
                }
                for i in 0..3 {
                    controls = (controls << 8) | vcd.data[9 - i] as u32;
                }
                println!("        bmControlsRuntime       0x{:08x}", controls);
                for (i, n) in EN_CTRL_NAMES.iter().enumerate().take(20) {
                    if (controls >> i) & 1 != 0 {
                        println!("         {}", n);
                    }
                }
            } else {
                println!("      Warning: Descriptor too short");
            }
        }
        _ => {
            println!("(unknown)");
            println!(
                "        Invalid desc subtype: {}",
                vcd.data
                    .iter()
                    .map(|b| format!("{:02x}", b))
                    .collect::<Vec<String>>()
                    .join(" ")
            );
        }
    }
}

fn dump_videostreaming_interface(gd: &GenericDescriptor) {
    println!("      VideoStreaming Interface Descriptor:");
    println!("        bLength              {:5}", gd.length);
    println!("        bDescriptorType      {:5}", gd.descriptor_type);
    print!("        bDescriptorSubtype   {:5} ", gd.descriptor_subtype);

    let color_primatives = |c: u8| match c {
        1 => "BT.709,sRGB",
        2 => "BT.470-2 (M)",
        3 => "BT.470-2 (B,G)",
        4 => "SMPTE 170M",
        5 => "SMPTE 240M",
        _ => "Unspecified",
    };

    let transfer_characteristics = |c: u8| match c {
        1 => "BT.709",
        2 => "BT.470-2 (M)",
        3 => "BT.470-2 (B,G)",
        4 => "SMPTE 170M",
        5 => "SMPTE 240M",
        6 => "Linear",
        7 => "sRGB",
        _ => "Unspecified",
    };

    let matrix_coefficients = |c: u8| match c {
        1 => "BT.709",
        2 => "FCC",
        3 => "BT.470-2 (B,G)",
        4 => "SMPTE 170M (BT.601)",
        5 => "SMPTE 240M",
        _ => "Unspecified",
    };

    let field_pattern = |f: u8| match f {
        0 => "Field 1 only",
        1 => "Field 2 only",
        2 => "Regular pattern of fields 1 and 2",
        3 => "Random pattern of fields 1 and 2",
        _ => "Invalid",
    };

    if let Some(data) = &gd.data {
        match gd.descriptor_subtype {
            0x01 => {
                println!("(INPUT_HEADER)");
                if data.len() >= 11 {
                    let formats = data[0];
                    let control_size = data[9];
                    println!("        bNumFormats                     {:5}", formats);
                    println!(
                        "        wTotalLength                   0x{:04x}",
                        u16::from_le_bytes([data[1], data[2]])
                    );
                    println!(
                        "        bEndpointAddress                 0x{:02x}  EP {} {}",
                        data[3],
                        data[3] & 0x0f,
                        if data[3] & 0x80 != 0 { "IN" } else { "OUT" }
                    );
                    println!("        bmInfo                          {:5}", data[4]);
                    println!("        bTerminalLink                   {:5}", data[5]);
                    println!("        bStillCaptureMethod             {:5}", data[6]);
                    println!("        bTriggerSupport                 {:5}", data[7]);
                    println!("        bTriggerUsage                   {:5}", data[8]);
                    println!("        bControlSize                    {:5}", control_size);
                    for (i, b) in data[10..].chunks(control_size as usize).enumerate() {
                        if i == formats as usize {
                            break;
                        }
                        println!("        bmaControls({:2})                 {:5}", i, b[0]);
                    }

                    dump_junk(
                        data,
                        8,
                        gd.expected_data_length(),
                        10 + formats as usize * control_size as usize,
                    );
                }
            }
            0x02 => {
                println!("(OUTPUT_HEADER)");
                if data.len() >= 7 {
                    let formats = data[0];
                    let control_size = data[8];
                    println!("        bNumFormats                     {:5}", formats);
                    println!(
                        "        wTotalLength                  0x{:04x}",
                        u16::from_le_bytes([data[1], data[2]])
                    );
                    println!(
                        "        bEndpointAddress                0x{:02x}  EP {} {}",
                        data[3],
                        data[3] & 0x0f,
                        if data[3] & 0x80 != 0 { "IN" } else { "OUT" }
                    );
                    println!("        bTerminalLink                   {:5}", data[4]);
                    println!("        bControlSize                    {:5}", control_size);
                    for (i, b) in data[6..].chunks(control_size as usize).enumerate() {
                        if i == formats as usize {
                            break;
                        }
                        println!("        bmaControls({:2})                 {:5}", i, b[0]);
                    }

                    dump_junk(
                        data,
                        8,
                        gd.expected_data_length(),
                        6 + formats as usize * control_size as usize,
                    );
                }
            }
            0x03 => {
                println!("(STILL_IMAGE_FRAME)");
                if data.len() >= 3 {
                    let image_num = data[1] as usize;
                    let compression_num = data[2 + image_num * 4];
                    println!(
                        "        bEndpointAddress              0x{:02x}  EP {} {}",
                        data[0],
                        data[0] & 0x0f,
                        if data[0] & 0x80 != 0 { "IN" } else { "OUT" }
                    );
                    println!("        bNumImageSizePatterns          {:3}", image_num);
                    for (i, b) in data[2..].chunks(4).enumerate() {
                        if i == image_num {
                            break;
                        }
                        println!(
                            "        wWidth({:2})                   {:5}",
                            i,
                            u16::from_le_bytes([b[0], b[1]])
                        );
                        println!(
                            "        wHeight({:2})                  {:5}",
                            i,
                            u16::from_le_bytes([b[2], b[3]])
                        );
                    }
                    println!(
                        "        bNumCompressionPatterns        {:3}",
                        compression_num
                    );
                    if data.len() >= 3 + image_num * 4 + compression_num as usize {
                        for (i, b) in data[3 + image_num * 4..].iter().enumerate() {
                            if i == compression_num as usize {
                                break;
                            }
                            println!("        bCompression({:2})             {:5}", i, b);
                        }
                    }

                    dump_junk(
                        data,
                        8,
                        gd.expected_data_length(),
                        3 + image_num * 4 + compression_num as usize,
                    );
                }
            }
            0x04 | 0x010 => {
                let len = if gd.descriptor_subtype == 0x04 {
                    println!("(FORMAT_UNCOMPRESSED)");
                    24
                } else {
                    println!("(FORMAT_FRAME_BASED)");
                    25
                };

                if data.len() >= len {
                    let flags = data[22];
                    println!("        bFormatIndex                    {:5}", data[0]);
                    println!("        bNumFrameDescriptors            {:5}", data[1]);
                    println!(
                        "        guidFormat                            {}",
                        get_guid(&data[2..18])
                    );
                    println!("        bBitsPerPixel                   {:5}", data[18]);
                    println!("        bDefaultFrameIndex              {:5}", data[19]);
                    println!("        bAspectRatioX                   {:5}", data[20]);
                    println!("        bAspectRatioY                   {:5}", data[21]);
                    println!("        bmInterlaceFlags                 0x{:02x}", flags);
                    println!("        bCopyProtect                    {:5}", data[23]);
                    println!(
                        "          Interlaced stream or variable: {}",
                        if flags & 0x01 != 0 { "Yes" } else { "No" }
                    );
                    println!(
                        "          Fields per frame: {}",
                        if flags & 0x02 != 0 { "1" } else { "2" }
                    );
                    println!(
                        "          Field 1 first: {}",
                        if flags & 0x04 != 0 { "Yes" } else { "No" }
                    );
                    println!(
                        "          Field pattern: {}",
                        field_pattern((flags >> 4) & 0x03)
                    );
                    if gd.descriptor_subtype == 0x10 {
                        println!(
                            "        bVariableSize                  {:5}",
                            data.get(24).unwrap_or(&0)
                        );
                    }
                }

                dump_junk(data, 8, gd.expected_data_length(), len);
            }
            0x05 | 0x07 | 0x11 => {
                let n = if gd.descriptor_subtype == 0x05 {
                    println!("(FRAME_UNCOMPRESSED)");
                    22
                } else if gd.descriptor_subtype == 0x07 {
                    println!("(FRAME_MJPEG)");
                    22
                } else {
                    println!("(FRAME_FRAME_BASED)");
                    18
                };

                if data.len() >= 23 {
                    let flags = data[1];
                    let len = if data[n] != 0 {
                        23 + data[n] as usize * 4
                    } else {
                        35
                    };
                    println!("        bFrameIndex                     {:5}", data[0]);
                    println!("        bmCapabilities                   0x{:02x}", flags);
                    if flags & 0x01 != 0 {
                        println!("          Still image supported");
                    } else {
                        println!("          Still image unsupported");
                    }
                    if flags & 0x02 != 0 {
                        println!("          Fixed frame-rate");
                    }
                    println!(
                        "        wWidth                          {:5}",
                        u16::from_le_bytes([data[2], data[3]])
                    );
                    println!(
                        "        wHeight                         {:5}",
                        u16::from_le_bytes([data[4], data[5]])
                    );
                    println!(
                        "        dwMinBitRate                {:9}",
                        u32::from_le_bytes([data[6], data[7], data[8], data[9]])
                    );
                    println!(
                        "        dwMaxBitRate                {:9}",
                        u32::from_le_bytes([data[10], data[11], data[12], data[13]])
                    );
                    if gd.descriptor_subtype == 0x11 {
                        println!(
                            "        dwDefaultFrameInterval      {:9}",
                            u32::from_le_bytes([data[14], data[15], data[16], data[17]])
                        );
                        println!("        bFrameIntervalType              {:5}", data[18]);
                        println!(
                            "        dwBytesPerLine              {:9}",
                            u32::from_le_bytes([data[19], data[20], data[21], data[22]])
                        );
                    } else {
                        println!(
                            "        dwMaxVideoFrameBufferSize   {:9}",
                            u32::from_le_bytes([data[14], data[15], data[16], data[17]])
                        );
                        println!(
                            "        dwDefaultFrameInterval      {:9}",
                            u32::from_le_bytes([data[18], data[19], data[20], data[21]])
                        );
                        println!("        bFrameIntervalType              {:5}", data[22]);
                    }
                    if data[n] == 0 && data.len() >= 35 {
                        println!(
                            "        dwMinFrameInterval          {:9}",
                            u32::from_le_bytes([data[23], data[24], data[25], data[26]])
                        );
                        println!(
                            "        dwMaxFrameInterval          {:9}",
                            u32::from_le_bytes([data[27], data[28], data[29], data[30]])
                        );
                        println!(
                            "        dwFrameIntervalStep         {:9}",
                            u32::from_le_bytes([data[31], data[32], data[33], data[34]])
                        );
                    } else {
                        for (i, b) in data[n..].chunks(4).enumerate() {
                            if i == data[n] as usize {
                                break;
                            }
                            println!(
                                "        dwFrameInterval({:2})       {:11}",
                                i,
                                u32::from_le_bytes([b[0], b[1], b[2], b[3]])
                            );
                        }
                    }

                    dump_junk(data, 8, gd.expected_data_length(), len);
                }
            }
            0x06 => {
                let mut flags = data[2];
                println!("(FORMAT_MJPEG)");
                if data.len() >= 8 {
                    println!("        bFormatIndex                    {:3}", data[0]);
                    println!("        bNumFrameDescriptors            {:3}", data[1]);
                    println!("        bFlags                          {:3}", flags);
                    println!(
                        "          Fixed-sized samples: {}",
                        if flags & 0x01 != 0 { "Yes" } else { "No" }
                    );
                    flags = data[6];
                    println!("        bDefaultFrameIndex              {:3}", data[3]);
                    println!("        bAspectRatioX                   {:3}", data[4]);
                    println!("        bAspectRatioY                   {:3}", data[5]);
                    println!("        bmInterlaceFlags               0x{:02x}", flags);
                    println!(
                        "          Interlaced stream or variable: {}",
                        if flags & 0x01 != 0 { "Yes" } else { "No" }
                    );
                    println!(
                        "          Fields per frame: {}",
                        if flags & 0x02 != 0 { "1" } else { "2" }
                    );
                    println!(
                        "          Field 1 first: {}",
                        if flags & 0x04 != 0 { "Yes" } else { "No" }
                    );
                    println!(
                        "          Field pattern: {}",
                        field_pattern((flags >> 4) & 0x03)
                    );
                    println!("        bCopyProtect                    {:3}", data[7]);
                }

                dump_junk(data, 8, gd.expected_data_length(), 8);
            }
            0x0a => {
                println!("(FORMAT_MPEG2TS)");
                if data.len() >= 4 {
                    println!("        bFormatIndex                    {:3}", data[0]);
                    println!("        bDataOffset                     {:3}", data[1]);
                    println!("        bPacketLength                   {:3}", data[2]);
                    println!("        bStrideLength                   {:3}", data[3]);
                    if data.len() >= 20 {
                        println!(
                            "        guidStrideFormat                      {}",
                            get_guid(&data[4..20])
                        );
                    }
                }

                if gd.len() < 23 {
                    dump_junk(data, 8, gd.expected_data_length(), 4);
                } else {
                    dump_junk(data, 8, gd.expected_data_length(), 20);
                }
            }
            0x0d => {
                println!("(COLORFORMAT)");
                if data.len() >= 3 {
                    println!(
                        "        bColorPrimaries                 {:3} ({})",
                        data[0],
                        color_primatives(data[0])
                    );
                    println!(
                        "        bTransferCharacteristics        {:3} ({})",
                        data[1],
                        transfer_characteristics(data[1])
                    );
                    println!(
                        "        bMatrixCoefficients             {:3} ({})",
                        data[2],
                        matrix_coefficients(data[2])
                    );
                }

                dump_junk(data, 8, gd.expected_data_length(), 3);
            }
            0x12 => {
                println!("(FORMAT_STREAM_BASED)");
                if data.len() >= 18 {
                    println!("        bFormatIndex                    {:3}", data[0]);
                    println!(
                        "        guidFormat                            {}",
                        get_guid(&data[1..17])
                    );
                    println!("        dwPacketLength                {:5}", data[17]);
                }

                dump_junk(data, 8, gd.expected_data_length(), 21);
            }
            _ => {
                println!("(unknown)");
                println!(
                    "        Invalid desc subtype: {}",
                    data.iter()
                        .map(|b| format!("{:02x}", b))
                        .collect::<Vec<String>>()
                        .join(" ")
                );
            }
        }
    }
}

fn dump_bad_comm(cd: &CommunicationDescriptor, indent: usize) {
    let data = Into::<Vec<u8>>::into(cd.to_owned());
    // convert to exact type str used by lsusb
    let type_str = match cd.communication_type {
        CdcType::Header => "Header",
        CdcType::CallManagement => "Call Management",
        CdcType::AbstractControlManagement => "ACM",
        CdcType::Union => "Union",
        CdcType::CountrySelection => "Country Selection",
        CdcType::TelephoneOperationalModes => "Telephone Operations",
        CdcType::NetworkChannel => "Network Channel Terminal",
        CdcType::EthernetNetworking => "Ethernet",
        CdcType::WirelessHandsetControlModel => "WHCM version",
        CdcType::MobileDirectLineModelFunctional => "MDLM",
        CdcType::MobileDirectLineModelDetail => "MDLM detail",
        CdcType::DeviceManagement => "Device Management",
        CdcType::Obex => "OBEX",
        CdcType::CommandSet => "Command Set",
        CdcType::Ncm => "NCM",
        CdcType::Mbim => "MBIM",
        CdcType::MbimExtended => "MBIM Extended",
        _ => "",
    };
    println!(
        "{:^indent$}INVALID CDC ({}): {}",
        "",
        type_str,
        data.iter()
            .map(|b| format!("{:02x}", b))
            .collect::<Vec<String>>()
            .join(" ")
    );
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

fn dump_comm_descriptor(cd: &CommunicationDescriptor, indent: usize) {
    match cd.communication_type {
        CdcType::Header => {
            if cd.data.len() >= 2 {
                println!("{:^indent$}CDC Header:", "");
                println!(
                    "{:^indent$}  bcdCDC              {:x}.{:02x}",
                    "", cd.data[1], cd.data[0]
                );
            } else {
                dump_bad_comm(cd, indent);
            }
        }
        CdcType::CallManagement => {
            if cd.data.len() >= 2 {
                println!("{:^indent$}CDC Call Management:", "");
                println!("{:^indent$}  bmCapabilities      0x{:02x}", "", cd.data[0]);
                if cd.data[0] & 0x01 != 0x00 {
                    println!("{:^indent$}    call management", "");
                }
                if cd.data[0] & 0x02 != 0x00 {
                    println!("{:^indent$}    use cd.dataInterface", "");
                }
            } else {
                dump_bad_comm(cd, indent);
            }
        }
        CdcType::AbstractControlManagement => {
            if !cd.data.is_empty() {
                println!("{:^indent$}CDC ACM:", "");
                println!("{:^indent$}  bmCapabilities      0x{:02x}", "", cd.data[0]);
                if cd.data[0] & 0x08 != 0x00 {
                    println!("{:^indent$}    connection notifications", "");
                }
                if cd.data[0] & 0x04 != 0x00 {
                    println!("{:^indent$}    sends break", "");
                }
                if cd.data[0] & 0x02 != 0x00 {
                    println!("{:^indent$}    line coding and serial state", "");
                }
                if cd.data[0] & 0x01 != 0x00 {
                    println!("{:^indent$}    get/set/clear comm features", "");
                }
            } else {
                dump_bad_comm(cd, indent);
            }
        }
        CdcType::Union => {
            if cd.data.len() >= 2 {
                println!("{:^indent$}CDC Union:", "");
                println!("{:^indent$}  bMasterInterface     {:3}", "", cd.data[0]);
                println!(
                    "{:^indent$}  bSlaveInterface      {}",
                    "",
                    cd.data[1..]
                        .iter()
                        .map(|b| format!("{:3}", b))
                        .collect::<Vec<String>>()
                        .join(" ")
                );
            } else {
                dump_bad_comm(cd, indent);
            }
        }
        CdcType::CountrySelection => {
            if cd.data.len() >= 3 || (cd.length & 0x01) != 0 {
                println!("{:^indent$}Country Selection:", "");
                println!(
                    "{:^indent$}  iCountryCodeRelDate     {:3} {}",
                    "",
                    cd.string_index.unwrap_or_default(),
                    cd.string.as_ref().unwrap_or(&String::from("(?)"))
                );
                cd.data.chunks(2).for_each(|d| {
                    println!(
                        "{:^indent$}  wCountryCode          {:02x}{:02x}",
                        "", d[0], d[1]
                    );
                });
            } else {
                dump_bad_comm(cd, indent);
            }
        }
        CdcType::TelephoneOperationalModes => {
            if !cd.data.is_empty() {
                println!("{:^indent$}CDC Telephone operations:", "");
                println!("{:^indent$}  bmCapabilities       0x{:02x}", "", cd.data[0]);
                if cd.data[0] & 0x04 != 0x00 {
                    println!("{:^indent$}    computer centric mode", "");
                }
                if cd.data[0] & 0x02 != 0x00 {
                    println!("{:^indent$}    standalone mode", "");
                }
                if cd.data[0] & 0x01 != 0x00 {
                    println!("{:^indent$}    simple mode", "");
                }
            } else {
                dump_bad_comm(cd, indent);
            }
        }
        CdcType::NetworkChannel => {
            if cd.data.len() >= 4 {
                println!("{:^indent$}Network Channel Terminal:", "");
                println!("{:^indent$}  bEntityId               {:3}", "", cd.data[0]);
                println!(
                    "{:^indent$}  iName                   {:3} {}",
                    "",
                    cd.string_index.unwrap_or_default(),
                    cd.string.as_ref().unwrap_or(&String::from("(?)"))
                );
                println!("{:^indent$}  bChannelIndex           {:3}", "", cd.data[2]);
                println!("{:^indent$}  bPhysicalInterface      {:3}", "", cd.data[3]);
            } else {
                dump_bad_comm(cd, indent);
            }
        }
        CdcType::EthernetNetworking => {
            if cd.data.len() >= 13 - 3 {
                println!("{:^indent$}CDC Ethernet:", "");
                println!(
                    "{:^indent$}  iMacAddress             {:10} {}",
                    "",
                    cd.string_index.unwrap_or_default(),
                    cd.string.as_ref().unwrap_or(&String::from("(?)"))
                );
                println!(
                    "{:^indent$}  bmEthernetStatistics    0x{:08x}",
                    "",
                    u32::from_le_bytes([cd.data[1], cd.data[2], cd.data[3], cd.data[4]])
                );
                println!(
                    "{:^indent$}  wMaxSegmentSize         {:10}",
                    "",
                    u16::from_le_bytes([cd.data[5], cd.data[6]])
                );
                println!(
                    "{:^indent$}  wNumberMCFilters            0x{:04x}",
                    "",
                    u16::from_le_bytes([cd.data[7], cd.data[8]])
                );
                println!("{:^indent$}  bNumberPowerFilters     {:10}", "", cd.data[9]);
            } else {
                dump_bad_comm(cd, indent);
            }
        }
        CdcType::WirelessHandsetControlModel => {
            if cd.data.len() >= 2 {
                println!("{:^indent$}CDC WHCM:", "");
                println!(
                    "{:^indent$}  bcdVersion           {:x}.{:02x}",
                    "", cd.data[1], cd.data[0]
                );
            } else {
                dump_bad_comm(cd, indent);
            }
        }
        CdcType::MobileDirectLineModelFunctional => {
            if cd.data.len() >= 18 {
                println!("{:^indent$}CDC MDLM:", "");
                println!(
                    "{:^indent$}  bcdCDC               {:x}.{:02x}",
                    "", cd.data[1], cd.data[0]
                );
                println!(
                    "{:^indent$}  bGUID               {}",
                    "",
                    get_guid(&cd.data[2..18])
                );
            } else {
                dump_bad_comm(cd, indent);
            }
        }
        CdcType::MobileDirectLineModelDetail => {
            if cd.data.len() >= 2 {
                println!("{:^indent$}CDC MDLM detail:", "");
                println!("{:^indent$}  bGuidDescriptorType  {:02x}", "", cd.data[0]);
                println!(
                    "{:^indent$}  bDetailData          {}",
                    "",
                    cd.data
                        .iter()
                        .map(|b| format!("{:02x}", b))
                        .collect::<Vec<String>>()
                        .join(" ")
                );
            } else {
                dump_bad_comm(cd, indent);
            }
        }
        CdcType::DeviceManagement => {
            if cd.data.len() >= 4 {
                println!("{:^indent$}CDC MDLM:", "");
                println!(
                    "{:^indent$}  bcdVersion           {:x}.{:02x}",
                    "", cd.data[1], cd.data[0]
                );
                println!(
                    "{:^indent$}  wMaxCommand          {:3}",
                    "",
                    u16::from_le_bytes([cd.data[2], cd.data[3]])
                );
            } else {
                dump_bad_comm(cd, indent);
            }
        }
        CdcType::Obex => {
            if cd.data.len() >= 2 {
                println!("{:^indent$}CDC OBEX:", "");
                println!(
                    "{:^indent$}  bcdVersion           {:x}.{:02x}",
                    "", cd.data[1], cd.data[0]
                );
            } else {
                dump_bad_comm(cd, indent);
            }
        }
        CdcType::CommandSet => {
            if cd.data.len() >= 19 {
                println!("{:^indent$}CDC Command Set:", "");
                println!(
                    "{:^indent$}  bcdVersion           {:x}.{:02x}",
                    "", cd.data[1], cd.data[0]
                );
                println!(
                    "{:^indent$}  iCommandSet          {:4} {}",
                    "",
                    cd.string_index.unwrap_or_default(),
                    cd.string.as_ref().unwrap_or(&String::from("(?)"))
                );
                println!(
                    "{:^indent$}  bGUID               {}",
                    "",
                    get_guid(&cd.data[3..19])
                );
            } else {
                dump_bad_comm(cd, indent);
            }
        }
        CdcType::Ncm => {
            if cd.data.len() >= 6 - 3 {
                println!("{:^indent$}CDC NCM:", "");
                println!(
                    "{:^indent$}  bcdNcmVersion        {:x}.{:02x}",
                    "", cd.data[1], cd.data[0]
                );
                println!(
                    "{:^indent$}  bmNetworkCapabilities 0x{:02x}",
                    "", cd.data[2]
                );
                if cd.data[2] & (1 << 5) != 0 {
                    println!("{:^indent$}    8-byte ntb input size", "");
                }
                if cd.data[2] & (1 << 4) != 0 {
                    println!("{:^indent$}    crc mode", "");
                }
                if cd.data[2] & (1 << 2) != 0 {
                    println!("{:^indent$}    max cd.datagram size", "");
                }
                if cd.data[2] & (1 << 2) != 0 {
                    println!("{:^indent$}    encapsulated commands", "");
                }
                if cd.data[2] & (1 << 1) != 0 {
                    println!("{:^indent$}    net address", "");
                }
                if cd.data[2] & (1 << 0) != 0 {
                    println!("{:^indent$}    packet filter", "");
                }
            } else {
                dump_bad_comm(cd, indent);
            }
        }
        CdcType::Mbim => {
            if cd.data.len() >= 9 {
                println!("{:^indent$}CDC MBIM:", "");
                println!(
                    "{:^indent$}  bcdMBIMVersion       {:x}.{:02x}",
                    "", cd.data[1], cd.data[0]
                );
                println!(
                    "{:^indent$}  wMaxControlMessage   {}",
                    "",
                    u16::from_le_bytes([cd.data[2], cd.data[3]])
                );
                println!("{:^indent$}  bNumberFilters       {}", "", cd.data[4]);
                println!("{:^indent$}  bMaxFilterSize       {}", "", cd.data[5]);
                println!(
                    "{:^indent$}  wMaxSegmentSize      {}",
                    "",
                    u16::from_le_bytes([cd.data[6], cd.data[7]])
                );
                println!(
                    "{:^indent$}  bmNetworkCapabilities 0x{:02x}",
                    "", cd.data[8]
                );
                if cd.data[8] & 0x20 != 0x00 {
                    println!("{:^indent$}    8-byte ntb input size", "");
                }
                if cd.data[8] & 0x08 != 0x00 {
                    println!("{:^indent$}    max cd.datagram size", "");
                }
            } else {
                dump_bad_comm(cd, indent);
            }
        }
        CdcType::MbimExtended => {
            if cd.data.len() >= 5 {
                println!("{:^indent$}CDC MBIM Extended:", "");
                println!(
                    "{:^indent$}  bcdMBIMExtendedVersion          {:x}.{:02x}",
                    "", cd.data[1], cd.data[0]
                );
                println!(
                    "{:^indent$}  bMaxOutstandingCommandMessages    {:3}",
                    "", cd.data[2]
                );
                println!(
                    "{:^indent$}  wMTU                            {:5}",
                    "",
                    u16::from_le_bytes([cd.data[3], cd.data[4]])
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

fn dump_dfu_interface(gd: &GenericDescriptor, indent: usize) {
    const DFU_WIDTH: usize = 36;

    dump_title("Device Firmware Upgrade Interface Descriptor:", indent);
    dump_value(gd.length, "bLength", indent + 2, DFU_WIDTH);
    dump_value(gd.descriptor_type, "bDescriptorType", indent + 2, DFU_WIDTH);
    dump_value(gd.descriptor_subtype, "bmAttributes", indent + 2, DFU_WIDTH);

    if gd.descriptor_subtype & 0xf0 != 0 {
        println!("{:indent$}(unknown attributes!)", "", indent = indent + 4);
    }
    if gd.descriptor_subtype & 0x08 != 0 {
        println!("{:indent$}Will Detach", "", indent = indent + 4);
    } else {
        println!("{:indent$}Will Not Detach", "", indent = indent + 4);
    }
    if gd.descriptor_subtype & 0x04 != 0 {
        println!("{:indent$}Manifestation Intolerant", "", indent = indent + 4);
    } else {
        println!("{:indent$}Manifestation Tolerant", "", indent = indent + 4);
    }
    if gd.descriptor_subtype & 0x02 != 0 {
        println!("{:indent$}Upload Supported", "", indent = indent + 4);
    } else {
        println!("{:indent$}Upload Unsupported", "", indent = indent + 4);
    }
    if gd.descriptor_subtype & 0x01 != 0 {
        println!("{:indent$}Download Supported", "", indent = indent + 4);
    } else {
        println!("{:indent$}Download Unsupported", "", indent = indent + 4);
    }

    if let Some(data) = &gd.data {
        if data.len() >= 4 {
            let detach_timeout = u16::from_le_bytes([data[0], data[1]]);
            dump_value_string(
                detach_timeout,
                "wDetachTimeout",
                "milliseconds",
                indent + 2,
                DFU_WIDTH,
            );
            let transfer_size = u16::from_le_bytes([data[2], data[3]]);
            dump_value_string(
                transfer_size,
                "wTransferSize",
                "bytes",
                indent + 2,
                DFU_WIDTH,
            );
        }
        if data.len() >= 6 {
            dump_value(
                format!("{:x}.{:02x}", data[5], data[4]),
                "bcdDFUVersion",
                indent + 2,
                DFU_WIDTH,
            );
        }
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
            "{:indent$}{} (0x{:02x})", "",
            subtype_string, gd.descriptor_subtype, indent = indent
        );
    } else {
        println!(
            "{:indent$}INTERFACE CLASS: {}", "",
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
    dump_title("Security Descriptor:", indent);
    dump_value(sec.length, "bLength", indent + 2, LSUSB_DUMP_WIDTH);
    dump_value(sec.descriptor_type, "bDescriptorType", indent + 2, LSUSB_DUMP_WIDTH);
    dump_hex(sec.total_length, "wTotalLength", indent + 2, LSUSB_DUMP_WIDTH);
    dump_value(sec.encryption_types, "bNumEncryptionTypes", indent + 2, LSUSB_DUMP_WIDTH);
}

fn dump_encryption_type(enc: &EncryptionDescriptor, indent: usize) {
    let enct_string = match enc.encryption_type as u8 {
        0 => "UNSECURE",
        1 => "WIRED",
        2 => "CCM_1",
        3 => "RSA_1",
        _ => "RESERVED",
    };

    dump_title("Encryption Type:", indent);
    dump_value(enc.length, "bLength", indent + 2, LSUSB_DUMP_WIDTH);
    dump_value(enc.descriptor_type, "bDescriptorType", indent + 2, LSUSB_DUMP_WIDTH);
    dump_value_string(
        enc.encryption_type as u8,
        "bEncryptionType",
        enct_string,
        indent + 2,
        LSUSB_DUMP_WIDTH,
    );
    dump_value(enc.encryption_value, "bEncryptionValue", indent + 2, LSUSB_DUMP_WIDTH);
    dump_value(enc.auth_key_index, "bAuthKeyIndex", indent + 2, LSUSB_DUMP_WIDTH);
}

fn dump_interface_association(iad: &InterfaceAssociationDescriptor, indent: usize) {
    dump_title("Interface Association:", indent);
    dump_value(iad.length, "bLength", indent + 2, LSUSB_DUMP_WIDTH);
    dump_value(iad.descriptor_type, "bDescriptorType", indent + 2, LSUSB_DUMP_WIDTH);
    dump_value(iad.first_interface, "bFirstInterface", indent + 2, LSUSB_DUMP_WIDTH);
    dump_value(iad.interface_count, "bInterfaceCount", indent + 2, LSUSB_DUMP_WIDTH);
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
    dump_title("HID Descriptor:", indent);
    dump_value(hidd.length, "bLength", indent + 2, LSUSB_DUMP_WIDTH);
    dump_value(hidd.descriptor_type, "bDescriptorType", indent + 2, LSUSB_DUMP_WIDTH);
    dump_value(hidd.bcd_hid, "bcdHID", indent + 2, LSUSB_DUMP_WIDTH);
    dump_value_string(
        hidd.country_code,
        "bCountryCode",
        names::countrycode(hidd.country_code).unwrap_or_default(),
        indent + 2,
        LSUSB_DUMP_WIDTH,
    );
    dump_value(hidd.descriptors.len(), "bNumDescriptors", indent + 2, LSUSB_DUMP_WIDTH);
    for desc in &hidd.descriptors {
        dump_value_string(
            desc.descriptor_type,
            "bDescriptorType",
            names::hid(desc.descriptor_type).unwrap_or_default(),
            indent + 2,
            LSUSB_DUMP_WIDTH,
        );
        dump_value(desc.length, "wDescriptorLength", indent + 2, LSUSB_DUMP_WIDTH);
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
                dump_title("Report Descriptors:", indent + 2);
                dump_title("** UNAVAILABLE **", indent + 4);
            }
        }
    }
}

/// Verbatum port of lsusb's dump_unit - not very Rust, don't judge!
fn dump_unit(mut data: u16, len: usize) {
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
            println!("System: Vendor defined, Unit: (unknown)");
        } else {
            println!("System: Reserved, Unit: (unknown)");
        }

        return;
    }

    print!("System: {}, Unit: ", systems(sys));

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
    const REPORT_INDENT: usize = 28;
    let types = |t: u8| match t {
        0x00 => "Main",
        0x01 => "Global",
        0x02 => "Local",
        _ => "reserved",
    };

    dump_title(&format!("Report Descriptor: (length is {})", desc.len()), indent);

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
                println!(
                    "{:indent$}{}",
                    "",
                    names::huts(hut).unwrap_or_default(),
                    indent = REPORT_INDENT
                );
            }
            // usage, usage minimum, usage maximum
            0x08 | 0x18 | 0x28 => {
                println!(
                    "{:indent$}{}",
                    "",
                    names::hutus(hut, data as u16).unwrap_or_default(),
                    indent = REPORT_INDENT
                );
            }
            // unit exponent
            0x54 => {
                println!(
                    "{:indent$}{}: {}",
                    "",
                    "Unit Exponent",
                    data as u8,
                    indent = REPORT_INDENT
                );
            }
            // unit
            0x64 => {
                print!("{:indent$}", "", indent = indent + 2);
                dump_unit(data as u16, bsize)
            }
            // collection
            0xa0 => match data {
                0x00 => println!("{:indent$}{}", "", "Physical", indent = REPORT_INDENT),
                0x01 => println!("{:indent$}{}", "", "Application", indent = REPORT_INDENT),
                0x02 => println!("{:indent$}{}", "", "Logical", indent = REPORT_INDENT),
                0x03 => println!("{:indent$}{}", "", "Report", indent = REPORT_INDENT),
                0x04 => println!("{:indent$}{}", "", "Named Array", indent = REPORT_INDENT),
                0x05 => println!("{:indent$}{}", "", "Usage Switch", indent = REPORT_INDENT),
                0x06 => println!("{:indent$}{}", "", "Usage Modifier", indent = REPORT_INDENT),
                _ => {
                    if (data & 0x80) == 0x80 {
                        println!("{:indent$}{}", "", "Vendor defined", indent = REPORT_INDENT)
                    } else {
                        println!("{:indent$}{}", "", "Unknown", indent = REPORT_INDENT)
                    }
                }
            },
            // input, output, feature
            0x80 | 0x90 | 0xb0 => {
                let attributes_1 = format!(
                    "{:indent$}{} {} {} {} {}",
                    "",
                    if data & 0x01 != 0 { "Constant" } else { "Data" },
                    if data & 0x02 != 0 { "Variable" } else { "Array" },
                    if data & 0x04 != 0 { "Relative" } else { "Absolute" },
                    if data & 0x08 != 0 { "Wrap" } else { "No_Wrap" },
                    if data & 0x10 != 0 { "Non_Linear" } else { "Linear" },
                    indent = REPORT_INDENT
                );

                let attributes_2 = format!(
                    "{:indent$}{} {} {} {}",
                    "",
                    if data & 0x20 != 0 { "No_Preferred_State" } else { "Preferred_State" },
                    if data & 0x40 != 0 { "Null_State" } else { "No_Null_Position" },
                    if data & 0x80 != 0 { "Volatile" } else { "Non_Volatile" },
                    if data & 0x100 != 0 { "Buffered Bytes" } else { "Bitfield" },
                    indent = REPORT_INDENT
                );
                println!("{}", attributes_1);
                println!("{}", attributes_2);
            },
            _ => (),
        }
        i += 1 + bsize;
    }
}
