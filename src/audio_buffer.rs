use std::ffi::c_void;
use std::marker::PhantomData;
use std::mem;
use std::ops::{Deref, DerefMut};

use coreaudio_sys::{AudioBuffer, AudioBufferList as SysAudioBufferList};

use crate::Sample;

//
pub struct AudioBufferList<S: Sample> {
    list: SysAudioBufferList,
    _data: Box<[S]>,
    _ph: PhantomData<S>,
}

impl<S: Sample> AudioBufferList<S> {
    pub fn new(channels: usize, size: usize) -> Self {
        let len = channels * size;
        let byte_size = len * mem::size_of::<S>();

        let mut data = (vec![S::default(); len]).into_boxed_slice();

        let buffer = AudioBuffer {
            mNumberChannels: channels as u32,
            mDataByteSize: byte_size as u32,
            mData: data.as_mut_ptr() as *mut c_void,
        };

        let list = SysAudioBufferList {
            mNumberBuffers: 1,
            mBuffers: [buffer],
        };

        Self {
            list,
            _data: data,
            _ph: PhantomData,
        }
    }
}

impl<S: Sample> Deref for AudioBufferList<S> {
    type Target = [S];

    fn deref(&self) -> &Self::Target {
        let len = self.list.mBuffers[0].mDataByteSize as usize / mem::size_of::<S>();
        unsafe { std::slice::from_raw_parts(self.list.mBuffers[0].mData as *const S, len) }
    }
}

impl<S: Sample> DerefMut for AudioBufferList<S> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        let len = self.list.mBuffers[0].mDataByteSize as usize / mem::size_of::<S>();
        unsafe { std::slice::from_raw_parts_mut(self.list.mBuffers[0].mData as *mut S, len) }
    }
}
