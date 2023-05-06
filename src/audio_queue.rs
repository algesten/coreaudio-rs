use std::ffi::c_void;
use std::marker::PhantomData;
use std::mem;
use std::ops::{Deref, DerefMut};
use std::ptr;
use std::sync::mpsc;

use coreaudio_sys::{
    AudioQueueAllocateBuffer, AudioQueueBufferRef, AudioQueueDispose, AudioQueueEnqueueBuffer,
    AudioQueueFreeBuffer, AudioQueueNewInput, AudioQueueNewOutput, AudioQueueRef, AudioQueueStart,
    AudioQueueStop, AudioStreamPacketDescription, AudioTimeStamp,
};

use crate::{try_os_status, Error, Sample, StreamFormat};

pub struct AudioQueueOutput<S: Sample> {
    queue_ref: AudioQueueRef,
    buffers: Vec<AudioQueueBuffer<S>>,
    next_buffer: mpsc::Receiver<usize>,
    tx: mpsc::Sender<usize>,
}

type OutputCallbackFn = dyn FnMut(AudioQueueBufferRef);

struct OutputCallbackWrapper {
    callback: Box<OutputCallbackFn>,
}

impl<S: Sample> AudioQueueOutput<S> {
    pub fn new(
        format: &StreamFormat,
        buffer_count: usize,
        buffer_size: usize,
    ) -> Result<Self, Error> {
        if S::sample_format() != format.sample_format {
            return Err(Error::SampleFormatDoesntMatchQueueType);
        }

        let mut queue_ref: AudioQueueRef = std::ptr::null_mut();
        let (tx, next_buffer) = mpsc::channel();

        let output_proc_fn = {
            let tx = tx.clone();
            move |buffer_ref: AudioQueueBufferRef| {
                let idx = unsafe { (*buffer_ref).mUserData as usize };
                tx.send(idx).ok();
            }
        };

        let wrapper = Box::new(OutputCallbackWrapper {
            callback: Box::new(output_proc_fn),
        });

        unsafe {
            try_os_status!(AudioQueueNewOutput(
                &format.to_asbd(),
                Some(output_proc),
                Box::into_raw(wrapper) as *mut c_void,
                ptr::null_mut(),
                ptr::null_mut(),
                0,
                &mut queue_ref,
            ));
        }

        let mut instance = Self {
            queue_ref,
            buffers: Vec::with_capacity(buffer_count),
            next_buffer,
            tx: tx.clone(),
        };

        for _ in 0..buffer_count {
            let idx = instance.allocate(buffer_size)?;
            tx.send(idx).expect("to enqueue index on creation");
        }

        Ok(instance)
    }

    pub fn start(&mut self) -> Result<(), Error> {
        unsafe { try_os_status!(AudioQueueStart(self.queue_ref, ptr::null_mut())) };
        Ok(())
    }

    pub fn stop(&mut self) -> Result<(), Error> {
        unsafe { try_os_status!(AudioQueueStop(self.queue_ref, 1)) };
        Ok(())
    }

    fn allocate(&mut self, max_len: usize) -> Result<usize, Error> {
        let idx = self.buffers.len();
        let buffer = AudioQueueBuffer::new(self.queue_ref, idx, max_len)?;
        self.buffers.push(buffer);
        Ok(idx)
    }

    pub fn request_buffer(&mut self) -> BorrowedAudioQueueBuffer<'_, S> {
        let index = self.next_buffer.recv().expect("next buffer index");
        BorrowedAudioQueueBuffer {
            output: self,
            index,
            was_enqueued: false,
        }
    }

    fn enqueue(&mut self, index: usize) -> Result<(), Error> {
        let buf = &self.buffers[index];
        unsafe {
            try_os_status!(AudioQueueEnqueueBuffer(
                self.queue_ref,
                buf.buffer_ref,
                0,
                ptr::null_mut()
            ))
        };
        Ok(())
    }
}

impl<S: Sample> Drop for AudioQueueOutput<S> {
    fn drop(&mut self) {
        let _ = self.stop();

        unsafe {
            AudioQueueDispose(self.queue_ref, 1);
        }
    }
}

pub struct AudioQueueInput<S: Sample> {
    queue_ref: AudioQueueRef,
    _ph: PhantomData<S>,
}

pub trait InputCallback<S> {
    fn audio_input(&mut self, start_time: AudioTimeStamp, buffer: &AudioQueueBuffer<S>);
}

type InputCallbackFn = dyn FnMut(AudioQueueRef, AudioQueueBufferRef, *const AudioTimeStamp);

struct InputCallbackWrapper {
    callback: Box<InputCallbackFn>,
}

impl<S: Sample> AudioQueueInput<S> {
    pub fn new<C: InputCallback<S> + 'static>(
        format: &StreamFormat,
        mut callback: C,
    ) -> Result<Self, Error> {
        if S::sample_format() != format.sample_format {
            return Err(Error::SampleFormatDoesntMatchQueueType);
        }

        let mut queue_ref: AudioQueueRef = ptr::null_mut();

        // This closure gets around the problem of having a generic S in the InputCallback.
        let input_proc_fn = move |queue_ref: AudioQueueRef,
                                  buffer_ref: AudioQueueBufferRef,
                                  start_time: *const AudioTimeStamp| {
            let buffer = AudioQueueBuffer::borrowed(queue_ref, buffer_ref);
            callback.audio_input(unsafe { *start_time }, &buffer);
        };

        let wrapper = Box::new(InputCallbackWrapper {
            callback: Box::new(input_proc_fn),
        });

        unsafe {
            try_os_status!(AudioQueueNewInput(
                &format.to_asbd(),
                Some(input_proc),
                Box::into_raw(wrapper) as *mut c_void,
                ptr::null_mut(),
                ptr::null_mut(),
                0,
                &mut queue_ref,
            ));
        }

        Ok(Self {
            queue_ref,
            _ph: PhantomData,
        })
    }

    pub fn start(&self) -> Result<(), Error> {
        unsafe { try_os_status!(AudioQueueStart(self.queue_ref, ptr::null_mut())) };

        Ok(())
    }

    pub fn stop(&self) -> Result<(), Error> {
        unsafe { try_os_status!(AudioQueueStop(self.queue_ref, 1)) };
        Ok(())
    }
}

pub struct BorrowedAudioQueueBuffer<'a, S: Sample> {
    output: &'a mut AudioQueueOutput<S>,
    index: usize,
    was_enqueued: bool,
}

impl<'a, S: Sample> BorrowedAudioQueueBuffer<'a, S> {
    pub fn enqueue(mut self) -> Result<(), Error> {
        self.was_enqueued = true;
        self.output.enqueue(self.index)?;
        Ok(())
    }
}

impl<'a, S: Sample> Drop for BorrowedAudioQueueBuffer<'a, S> {
    fn drop(&mut self) {
        if !self.was_enqueued {
            // Release straight away if buffer wasn't enqueued.
            self.output.tx.send(self.index).ok();
        }
    }
}

impl<'a, S: Sample> Deref for BorrowedAudioQueueBuffer<'a, S> {
    type Target = AudioQueueBuffer<S>;

    fn deref(&self) -> &Self::Target {
        &self.output.buffers[self.index]
    }
}

impl<'a, S: Sample> DerefMut for BorrowedAudioQueueBuffer<'a, S> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.output.buffers[self.index]
    }
}

pub struct AudioQueueBuffer<S> {
    queue_ref: AudioQueueRef,
    buffer_ref: AudioQueueBufferRef,
    free_on_drop: bool,
    _ph: PhantomData<S>,
}

impl<S> AudioQueueBuffer<S> {
    fn new(queue_ref: AudioQueueRef, idx: usize, len: usize) -> Result<AudioQueueBuffer<S>, Error> {
        let size = len * mem::size_of::<S>();
        let mut buffer_ref: AudioQueueBufferRef = ptr::null_mut();

        unsafe {
            try_os_status!(AudioQueueAllocateBuffer(
                queue_ref,
                size as u32,
                &mut buffer_ref
            ));

            // this is just an index so we know which buffer is which.
            (*buffer_ref).mUserData = idx as *mut c_void;
        }

        Ok(AudioQueueBuffer {
            queue_ref,
            buffer_ref,
            free_on_drop: true,
            _ph: PhantomData,
        })
    }

    fn borrowed(queue_ref: AudioQueueRef, buffer_ref: AudioQueueBufferRef) -> Self {
        AudioQueueBuffer {
            queue_ref,
            buffer_ref,
            free_on_drop: false,
            _ph: PhantomData,
        }
    }

    pub fn resize(&mut self, len: usize) {
        let max_bytes = unsafe { (*self.buffer_ref).mAudioDataBytesCapacity } as usize;
        let max = max_bytes / mem::size_of::<S>();
        let clamped = len.clamp(0, max);
        let byte_size = clamped * mem::size_of::<S>();
        unsafe { (*self.buffer_ref).mAudioDataByteSize = byte_size as u32 };
    }
}

impl<S> Drop for AudioQueueBuffer<S> {
    fn drop(&mut self) {
        if !self.free_on_drop {
            return;
        }

        unsafe {
            // ignore errors
            AudioQueueFreeBuffer(self.queue_ref, self.buffer_ref);
        }
    }
}

impl<S> Deref for AudioQueueBuffer<S> {
    type Target = [S];

    fn deref(&self) -> &Self::Target {
        let len = unsafe { (*self.buffer_ref).mAudioDataByteSize } as usize / mem::size_of::<S>();
        unsafe { std::slice::from_raw_parts((*self.buffer_ref).mAudioData as *mut S, len) }
    }
}

impl<S> DerefMut for AudioQueueBuffer<S> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        let len = unsafe { (*self.buffer_ref).mAudioDataByteSize } as usize / mem::size_of::<S>();
        unsafe { std::slice::from_raw_parts_mut((*self.buffer_ref).mAudioData as *mut S, len) }
    }
}

unsafe extern "C" fn output_proc(
    user_data: *mut c_void,
    _queue_ref: AudioQueueRef,
    buffer_ref: AudioQueueBufferRef,
) {
    let wrapper = user_data as *mut OutputCallbackWrapper;
    ((*wrapper).callback)(buffer_ref);
}

unsafe extern "C" fn input_proc(
    user_data: *mut c_void,
    queue_ref: AudioQueueRef,
    buffer_ref: AudioQueueBufferRef,
    start_time: *const AudioTimeStamp,
    _: u32,
    _: *const AudioStreamPacketDescription,
) {
    let wrapper = user_data as *mut InputCallbackWrapper;
    ((*wrapper).callback)(queue_ref, buffer_ref, start_time);
}

#[cfg(test)]
mod test {
    use std::f32::consts::PI;

    use crate::{LinearPcmFlags, SampleFormat};

    use super::*;

    #[test]
    fn test_queue_output() {
        let mut q = AudioQueueOutput::<f32>::new(
            &StreamFormat {
                sample_rate: 48_000.0,
                sample_format: SampleFormat::F32,
                flags: LinearPcmFlags::IS_FLOAT,
                channels: 1,
            },
            10,
            256,
        )
        .unwrap();

        q.start().unwrap();

        let angular_frequency = 2.0 * PI * 440.0;
        let sample_period = 1.0 / 48_000.0;
        let mut i = 0;

        loop {
            let mut buf = q.request_buffer();
            buf.resize(128);

            for sample in buf.iter_mut() {
                *sample = (angular_frequency * i as f32 * sample_period).sin();
                i += 1;
            }

            buf.enqueue().unwrap();
        }
    }
}
