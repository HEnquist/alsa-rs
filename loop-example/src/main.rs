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
        hwp.set_access(Access::MMapInterleaved)?;
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
        swp.set_avail_min(periodsize)?;
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
        hwp.set_access(Access::MMapInterleaved)?;
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
        swp.set_avail_min(periodsize)?;
        pcmdev.sw_params(&swp)?;
        println!("Opened audio output {:?} with parameters: {:?}, {:?}", req_devname, hwp, swp);
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

fn write_samples(pcmdev: &alsa::PCM, mmap: &mut MmapPlayback<SF>, chunk: AudioChunk) -> Res<bool> {

    //let mut chunk_iter = chunk.waveforms.iter().flat_map(|w| w.samples.iter().map(|s| *s));
    // Treat our 6-element array as a 2D 3x2 array, and transpose it to a 2x3 array
    //let mut output_array = vec![0; 6];
    //transpose::transpose(&input_array, &mut output_array, 3, 2);
    //let mut chunk_iter = chunk.waveforms.transpose().
    if mmap.avail() > 0 {
        // Write samples to DMA area from iterator
        mmap.write(&mut chunk.into_iter());
    }
    match mmap.status().state() {
        State::Running => { return Ok(false); }, // All fine
        State::Prepared => { println!("Starting audio output stream"); pcmdev.start()? },
        State::XRun => { println!("Underrun in audio output stream!"); pcmdev.prepare()? },
        State::Suspended => { println!("Resuming audio output stream"); pcmdev.resume()? },
        n @ _ => Err(format!("Unexpected pcm state {:?}", n))?,
    }
    Ok(true) // Call us again, please, there might be more data to write
}




fn run() -> Res<()> {
    let (playback_dev, pr_rate) = open_audio_dev_play("hw:PCH".to_string(), 44100, 256)?;
    let (capture_dev, cap_rate) = open_audio_dev_capt("hw:PCH".to_string(), 44100, 256)?;
    // Let's use the fancy new "direct mode" for minimum overhead!
    let mut mmap = playback_dev.direct_mmap_playback::<SF>()?;

    loop {
        //let wf_r = Waveform {
        //    len: 8,
        //    samples: vec![-100, -100, -100, -100, 100, 100, 100, 100],
        //};
        //let wf_l = Waveform {
        //    len: 8,
        //    samples: vec![-100, -100, -100, -100, 100, 100, 100, 100],
        //};
        let chunk = AudioChunk{
            waveforms: vec![vec![-100, -100, -100, -100, 0, 0, 0, 0, 100, 100, 100, 100, 0, 0, 0, 0],
                            vec![-100, -100, -100, -100, 0, 0, 0, 0, 100, 100, 100, 100, 0, 0, 0, 0]],
        };

        //}
        //loop {
        write_samples(&playback_dev, &mut mmap, chunk)?;
        //}
    }
    Ok(())
}

fn main() {
    if let Err(e) = run() { println!("Error ({}) {}", e.description(), e); }
}
