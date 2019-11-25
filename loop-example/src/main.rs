// A quickly made Hammond organ.

extern crate alsa;
//extern crate itertools;
//extern crate transpose;
//extern crate sample;

use std::{iter, error};
//use alsa::pcm;
//use std::ffi::CString;
//use sample::signal;
use alsa::{Direction, ValueOr};
use alsa::pcm::{PCM, HwParams, Format, Access, State};
use alsa::direct::pcm::MmapPlayback;
//use itertools::multizip;
//use transpose::transpose;
use std::{thread, time};



type Res<T> = Result<T, Box<dyn error::Error>>;




fn open_audio_dev_play(req_devname: String, req_samplerate: u32, req_bufsize: i64) -> Res<(alsa::PCM, u32)> {

    // Open the device
    let pcmdev = alsa::PCM::new(&req_devname, Direction::Playback, false)?;

    // Set hardware parameters
    {
        let hwp = HwParams::any(&pcmdev)?;
        hwp.set_channels(2)?;
        hwp.set_rate(req_samplerate, ValueOr::Nearest)?;
        hwp.set_format(Format::s16())?;
        hwp.set_access(Access::RWInterleaved)?;
        hwp.set_buffer_size(req_bufsize)?;
        hwp.set_period_size(req_bufsize / 4, alsa::ValueOr::Nearest)?;
        pcmdev.hw_params(&hwp)?;
    }

    // Set software parameters
    let rate = {
        let hwp = pcmdev.hw_params_current()?;
        let swp = pcmdev.sw_params_current()?;
        let (bufsize, periodsize) = (hwp.get_buffer_size()?, hwp.get_period_size()?);
        swp.set_start_threshold(bufsize - periodsize)?;
        //swp.set_avail_min(periodsize)?;
        pcmdev.sw_params(&swp)?;
        println!("Opened audio output {:?} with parameters: {:?}, {:?}", req_devname, hwp, swp);
        hwp.get_rate()?
    };

    Ok((pcmdev, rate))
}

fn open_audio_dev_capt(req_devname: String, req_samplerate: u32, req_bufsize: i64) -> Res<(alsa::PCM, u32)> {

    // Open the device
    let pcmdev = alsa::PCM::new(&req_devname, Direction::Capture, false)?;

    // Set hardware parameters
    {
        let hwp = HwParams::any(&pcmdev)?;
        hwp.set_channels(2)?;
        hwp.set_rate(req_samplerate, ValueOr::Nearest)?;
        hwp.set_format(Format::s16())?;
        hwp.set_access(Access::RWInterleaved)?;
        hwp.set_buffer_size(req_bufsize)?;
        hwp.set_period_size(req_bufsize / 4, alsa::ValueOr::Nearest)?;
        pcmdev.hw_params(&hwp)?;
    }

    // Set software parameters
    let rate = {
        let hwp = pcmdev.hw_params_current()?;
        let swp = pcmdev.sw_params_current()?;
        let (bufsize, periodsize) = (hwp.get_buffer_size()?, hwp.get_period_size()?);
        swp.set_start_threshold(bufsize - periodsize)?;
        //swp.set_avail_min(periodsize)?;
        pcmdev.sw_params(&swp)?;
        println!("Opened audio input {:?} with parameters: {:?}, {:?}", req_devname, hwp, swp);
        hwp.get_rate()?
    };

    Ok((pcmdev, rate))
}

// Sample format
type SF = i16;

//struct Waveform {
//    len: usize,
//    samples: Vec<SF>,
//}

struct AudioChunk {
    frames: usize,
    channels: usize,
    waveforms: Vec<Vec<SF>>, //Waveform>,
}

struct AudioChunkInterleaved {
    chunk: AudioChunk,
    index_time: usize,
    index_chan: usize,
}

impl Iterator for AudioChunkInterleaved {
    type Item = SF;
    fn next(&mut self) -> Option<SF> {
        if self.index_time>=self.chunk.waveforms[0].len() {
            return None
        }
        let result = self.chunk.waveforms[self.index_chan][self.index_time];
        self.index_chan += 1;
        if self.index_chan>=self.chunk.waveforms.len() {
            self.index_chan = 0;
            self.index_time += 1;
        }
        Some(result)
    }
}

impl IntoIterator for AudioChunk {
    type Item = SF;
    type IntoIter = AudioChunkInterleaved;

    fn into_iter(self) -> Self::IntoIter {
        AudioChunkInterleaved {
            chunk: self,
            index_time: 0,
            index_chan: 0,
        }
    }
}

impl AudioChunk {
    fn to_interleaved(self) -> Vec<SF> {
        //let buf = chunk.into_iter().collect::<Vec<SF>>();
        //buf
        let num_samples = self.channels*self.frames;
        let mut buf = Vec::with_capacity(num_samples);

        for frame in 0..self.frames {
            for chan in 0..self.channels {
                buf.push(self.waveforms[chan][frame]);
            }

        }
        buf
    }

        fn from_interleaved(buffer: Vec<SF>, num_channels: usize) -> AudioChunk {
        //let buf = chunk.into_iter().collect::<Vec<SF>>();
        //buf
        let num_samples = buffer.len();
        let num_frames = num_samples/num_channels;
        
        let mut waveforms = Vec::with_capacity(num_channels);
        for chan in 0..num_channels {
            waveforms.push(Vec::with_capacity(num_frames));
        }
        
        let mut samples = buffer.iter();
        for frame in 0..num_frames {
            for chan in 0..num_channels {
                waveforms[chan].push(*samples.next().unwrap());
            }

        }
        AudioChunk {
            channels: num_channels,
            frames: num_frames,
            waveforms: waveforms,
        }
    }
}


fn write_chunk(pcmdev: &alsa::PCM, io: &mut alsa::pcm::IO<SF>, chunk: AudioChunk) -> Res<usize> {
    //let buf = chunk.into_iter().collect::<Vec<SF>>();
    let buf = chunk.to_interleaved();
    let frames = io.writei(&buf[..])?;
    Ok(frames)
}

fn read_chunk(pcmdev: &alsa::PCM, io: &mut alsa::pcm::IO<SF>) -> Res<usize> {
    //let buf = chunk.into_iter().collect::<Vec<SF>>();
    let mut buf = vec![0i16; 2*1024];
    let frames = io.readi(&mut buf)?;
    Ok(frames)
}


fn run() -> Res<()> {
    let (playback_dev, play_rate) = open_audio_dev_play("hw:PCH".to_string(), 44100, 1024)?;
    let (capture_dev, capt_rate) = open_audio_dev_capt("hw:PCH".to_string(), 44100, 1024)?;

    
    
    //let mut mmap = playback_dev.direct_mmap_playback::<SF>()?;

    thread::spawn(move || {
        let mut io_play = playback_dev.io_i16().unwrap();
        for m in 0..2*44100/1024 {
            let mut buf = vec![0i16; 1024];
            for (i, a) in buf.iter_mut().enumerate() {
                *a = ((i as f32 * 2.0 * ::std::f32::consts::PI / 128.0).sin() * 8192.0) as i16
            }
            let chunk = AudioChunk{
                frames: 1024,
                channels: 2,
                waveforms: vec![buf.clone(),
                                buf],
            };

            //}
            //loop {
            //let frames_capt = 0;
            let playback_state = playback_dev.state();
            println!("playback state {:?}", playback_state);
            if playback_state == State::XRun {
                println!("Prepare playback");
                playback_dev.prepare().unwrap();
            }
            let frames = write_chunk(&playback_dev, &mut io_play, chunk).unwrap();

            println!("Chunk {}, wrote {} frames", m, frames);
        }
    });

    thread::spawn(move || {
        let mut io_capt = capture_dev.io_i16().unwrap();
        for m in 0..2*44100/1024 {
            let capture_state = capture_dev.state();
            println!("capture state {:?}", capture_state);
            if capture_state == State::XRun {
                println!("Prepare capture");
                capture_dev.prepare().unwrap();
            }
            let frames_capt = read_chunk(&capture_dev, &mut io_capt).unwrap();

            //println!("state {:?}", playback_dev.state());
            //if playback_dev.state() != State::Running { 
            //    playback_dev.start()?;
            //    println!("started");

            //}
            //}
            println!("Chunk {}, read {} frames", m, frames_capt);
        }
    });

    let delay = time::Duration::from_millis(100);
    

    loop {
        thread::sleep(delay);
    }
    Ok(())
}

fn main() {
    if let Err(e) = run() { println!("Error ({}) {}", e.description(), e); }
}
