use std::ffi::{c_uint, CStr};
use std::fmt;
use std::ptr;

use sys::{
    self, kCFStringEncodingUTF8, AudioComponentCopyName, AudioComponentGetDescription,
    AudioComponentGetVersion, CFStringGetCString, CFStringGetLength, CFStringRef,
};

use crate::Error;

use super::Type;

macro_rules! try_os_status {
    ($expr:expr) => {
        Error::from_os_status($expr)?
    };
}

#[derive(Clone)]
pub struct AudioUnitInfo {
    pub name: String,
    pub version: AudioUnitVersion,
    pub description: sys::AudioComponentDescription,
}

#[derive(Clone)]
pub struct AudioUnitVersion {
    pub major: u8,
    pub minor: u8,
    pub bugfix: u8,
    pub stage: u8,
}

pub fn list_unit_info(ty: Type) -> Result<Vec<AudioUnitInfo>, Error> {
    let au_type = ty.as_u32();
    let sub_type = match ty.as_subtype_u32() {
        Some(u) => u,
        None => 0, // Type has no subtype.
    };

    // A description of the audio unit we desire.
    let search_desc = sys::AudioComponentDescription {
        componentType: au_type as c_uint,
        componentSubType: sub_type,
        componentManufacturer: 0,
        componentFlags: 0,
        componentFlagsMask: 0,
    };

    let mut ret = Vec::new();

    unsafe {
        let mut component = ptr::null_mut();

        loop {
            component = sys::AudioComponentFindNext(component, &search_desc as *const _);
            if component.is_null() {
                break;
            }

            let mut name_ref: CFStringRef = std::ptr::null();
            try_os_status!(AudioComponentCopyName(component, &mut name_ref));
            let name = cfstring_ref_to_string(name_ref);

            let mut version = 0_u32;
            try_os_status!(AudioComponentGetVersion(component, &mut version));
            let major = ((version >> 24) & 0xff) as u8;
            let minor = ((version >> 16) & 0xff) as u8;
            let bugfix = ((version >> 8) & 0xff) as u8;
            let stage = (version & 0xff) as u8;
            let version = AudioUnitVersion {
                major,
                minor,
                bugfix,
                stage,
            };

            let mut description = sys::AudioComponentDescription::default();
            try_os_status!(AudioComponentGetDescription(component, &mut description));

            ret.push(AudioUnitInfo {
                name,
                version,
                description,
            });
        }
    }

    Ok(ret)
}

unsafe fn cfstring_ref_to_string(r: CFStringRef) -> String {
    let len = CFStringGetLength(r) + 1;
    let mut bytes = vec![0_i8; len as usize];

    CFStringGetCString(r, bytes.as_mut_ptr(), len, kCFStringEncodingUTF8);

    let c_str = CStr::from_ptr(bytes.as_ptr());
    c_str.to_str().unwrap().to_owned()
}

impl fmt::Debug for AudioUnitInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AudioUnitInfo")
            .field("name", &self.name)
            .field("version", &self.version)
            .finish()
    }
}

impl fmt::Debug for AudioUnitVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}.{}.{}.{}",
            self.major, self.minor, self.bugfix, self.stage
        )
    }
}

#[cfg(test)]
mod test {
    use crate::audio_unit::EffectType;

    use super::*;

    #[test]
    fn list_units_test() {
        let units = list_unit_info(Type::Effect(EffectType::None)).unwrap();

        println!("{:?}", units);
    }
}

// use coreaudio::audio_unit::{AudioUnit, Element, Scope};
// use coreaudio::sys::{
//     AudioFormatFlags, AudioStreamBasicDescription, AudioUnitGetProperty, AudioUnitGetPropertyInfo,
// };
// use std::mem;

// fn main() {
//     // Create an audio unit instance
//     let audio_unit = AudioUnit::new("aufx", "dely", "").unwrap();

//     // Get the input scope and element
//     let input_scope = Scope::Input;
//     let element = Element::from_scope(input_scope, 0).unwrap();

//     // Get the property ID for the supported formats
//     let property_id = kAudioUnitProperty_StreamFormat;

//     // Get the size of the property value
//     let mut data_size = 0;
//     let mut is_writable = 0;
//     unsafe {
//         AudioUnitGetPropertyInfo(
//             audio_unit.as_sys(),
//             property_id,
//             input_scope as u32,
//             element.to_u32(),
//             &mut data_size,
//             &mut is_writable,
//         )
//         .unwrap();
//     }

//     // Allocate memory for the property value
//     let mut supported_formats = Vec::<AudioStreamBasicDescription>::with_capacity(
//         data_size as usize / mem::size_of::<AudioStreamBasicDescription>(),
//     );

//     // Get the supported formats
//     unsafe {
//         AudioUnitGetProperty(
//             audio_unit.as_sys(),
//             property_id,
//             input_scope as u32,
//             element.to_u32(),
//             supported_formats.as_mut_ptr() as *mut _,
//             &mut data_size,
//         )
//         .unwrap();
//         supported_formats.set_len(data_size as usize / mem::size_of::<AudioStreamBasicDescription>());
//     }

//     // Loop through the supported formats and print information about each one
//     for (i, format) in supported_formats.iter().enumerate() {
//         println!(
//             "Supported format {}: {} channels, {} bits per channel, {} sample rate",
//             i + 1,
//             format.mChannelsPerFrame,
//             format.mBitsPerChannel,
//             format.mSampleRate
//         );
//     }
// }
