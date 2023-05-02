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
pub struct AudioUnitDescription {
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

pub fn list_units(ty: Type) -> Result<Vec<AudioUnitDescription>, Error> {
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

            ret.push(AudioUnitDescription {
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

impl fmt::Debug for AudioUnitDescription {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AudioUnitDescription")
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
        let units = list_units(Type::Effect(EffectType::None)).unwrap();

        println!("{:?}", units);
    }
}
