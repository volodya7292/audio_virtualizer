pub struct AudioDataRef<'a> {
    pub data: &'a [f32],
    num_channels: usize,
}

impl<'a> AudioDataRef<'a> {
    pub fn new(data: &'a [f32], num_channels: usize) -> Self {
        assert!(
            data.len() % num_channels == 0,
            "Data length must be a multiple of the number of channels."
        );
        Self { data, num_channels }
    }

    pub fn num_channels(&self) -> usize {
        self.num_channels
    }

    pub fn select_channel(&self, ch_idx: usize) -> impl Iterator<Item = &'a f32> {
        assert!(ch_idx < self.num_channels, "channel index out of bounds");
        self.data.iter().skip(ch_idx).step_by(self.num_channels)
    }
}

pub struct AudioDataMut<'a> {
    pub data: &'a mut [f32],
    num_channels: usize,
}

impl<'a> AudioDataMut<'a> {
    pub fn new(data: &'a mut [f32], num_channels: usize) -> Self {
        assert!(
            data.len() % num_channels == 0,
            "Data length must be a multiple of the number of channels."
        );
        Self { data, num_channels }
    }

    pub fn select_channel(&self, ch_idx: usize) -> impl Iterator<Item = &f32> {
        self.data.iter().skip(ch_idx).step_by(self.num_channels)
    }

    pub fn select_channel_mut(&mut self, ch_idx: usize) -> impl Iterator<Item = &mut f32> {
        self.data.iter_mut().skip(ch_idx).step_by(self.num_channels)
    }

    pub fn copy_channel_to_slice(&self, ch_idx: usize, other: &mut [f32]) {
        for (buf_v, s) in self.select_channel(ch_idx).zip(other) {
            *s = *buf_v;
        }
    }

    pub fn copy_channel_from_slice(&mut self, ch_idx: usize, other: &[f32]) {
        for (s, buf_v) in self.select_channel_mut(ch_idx).zip(other) {
            *s = *buf_v;
        }
    }
}

pub type AFrame<const CH: usize> = [f32; CH];
