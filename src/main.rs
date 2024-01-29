use std::{
    fs,
    io,
    env,
    thread,
    process,
    fmt::Display,
    path::Path,
    time::Duration,
    ops::{Index, IndexMut}
};

use sdl2::{
    EventPump,
    rect::Rect,
    pixels::Color,
    event::Event,
    video::Window
};

use config::Config;

mod config;


pub fn complain(message: impl Display) -> !
{
    println!("{message}");

    process::exit(1)
}

struct DrawerWindow
{
    window: Window,
    events: EventPump
}

impl DrawerWindow
{
    pub fn new(image: Image) -> Self
    {
        let ctx = sdl2::init().unwrap();

        let video = ctx.video().unwrap();

        let window = video.window("imagedisplay thingy!", image.width as u32, image.height as u32)
            .build()
            .unwrap();

        let events = ctx.event_pump().unwrap();

        let mut surface = window.surface(&events).unwrap();

        let mut draw_pixel = |x, y, c|
        {
            surface.fill_rect(Rect::new(x as i32, y as i32, 1, 1), c).unwrap();
        };

        for (i, pixel) in image.data.into_iter().enumerate()
        {
            let x = i % image.width;
            let y = i / image.width;

            draw_pixel(x, y, pixel);
        }

        surface.update_window().unwrap();

        Self{window, events}
    }

    pub fn wait_exit(mut self)
    {
        loop
        {
            for event in self.events.poll_iter()
            {
                match event
                {
                    Event::Quit{..} => return,
                    _ => ()
                }
            }

            let surface = self.window.surface(&self.events).unwrap();

            surface.update_window().unwrap();

            thread::sleep(Duration::from_millis(1000 / 60));
        }
    }
}

struct Image
{
    data: Vec<Color>,
    width: usize,
    height: usize
}

impl Image
{
    pub fn parse(
        path: impl AsRef<Path>,
        width: usize,
        c: Color,
        trim_start: usize,
        trim_end: usize
    ) -> Self
    {
        let values = fs::read(path).unwrap();

        let bpp = 3;
        let mut data: Vec<Color> = values[trim_start..(values.len() - trim_end)]
            .chunks(bpp).map(|chunk|
            {
                let r = chunk[0];
                let g = chunk.get(1).copied().unwrap_or(c.g);
                let b = chunk.get(2).copied().unwrap_or(c.b);

                Color::RGB(r, g, b)
            }).collect();

        // ceil integer div
        let height = {
            let l = data.len();
            let mut value = l / width;

            if l % width != 0
            {
                value += 1;
            }

            value
        };

        let total = width * height;
        if total < data.len()
        {
            panic!("total should never be less than len so far");
        }

        eprintln!("total amount of pixels: {}, total amount of bytes: {}", total, total * bpp);

        data.resize(total, c);

        Self{
            data,
            width,
            height
        }
    }

    pub fn save(&self, path: impl AsRef<Path>) -> io::Result<()>
    {
        let s = self.data.iter().flat_map(|c|
        {
            [c.r, c.g, c.b]
        }).collect::<Vec<u8>>();

        fs::write(path, s)
    }

    pub fn unhilbertify(&mut self)
    {
        assert_eq!(self.width, self.height);

        let size = self.width;
        let curve = HilbertCurve::new(size);

        self.remap_positions(|index|
        {
            let pos = curve.value_to_point(index);

            Self::to_index_assoc(size, pos)
        });
    }

    pub fn hilbertify(&mut self)
    {
        assert_eq!(self.width, self.height);

        let size = self.width;
        let curve = HilbertCurve::new(size);

        self.remap_positions(|index|
        {
            let pos = Self::index_to_pos_assoc(size, index);

            curve.point_to_value(pos)
        });
    }

    fn remap_positions(&mut self, mut f: impl FnMut(usize) -> usize)
    {
        let mut output = self.data.clone();

        self.data.iter().enumerate().for_each(|(i, value)|
        {
            let new_position = f(i);

            output[new_position] = *value;
        });

        self.data = output;
    }

    pub fn to_index(&self, pos: Pos2<usize>) -> usize
    {
        Self::to_index_assoc(self.width, pos)
    }

    pub fn to_index_assoc(width: usize, pos: Pos2<usize>) -> usize
    {
        pos.y * width + pos.x
    }

    pub fn index_to_pos_assoc(width: usize, index: usize) -> Pos2<usize>
    {
        Pos2{
            x: index % width,
            y: index / width
        }
    }
}

impl Index<Pos2<usize>> for Image
{
    type Output = Color;

    fn index(&self, index: Pos2<usize>) -> &Self::Output
    {
        &self.data[self.to_index(index)]
    }
}

impl IndexMut<Pos2<usize>> for Image
{
    fn index_mut(&mut self, index: Pos2<usize>) -> &mut Self::Output
    {
        let index = self.to_index(index);

        &mut self.data[index]
    }
}

#[derive(Debug, Copy, Clone)]
struct Pos2<T>
{
    x: T,
    y: T
}

struct HilbertCurve
{
    order: usize
}

impl HilbertCurve
{
    pub fn new(size: usize) -> Self
    {
        let mut order = 0;

        let mut current = size;
        while current > 0
        {
            current /= 2;

            order += 1;
        }

        order -= 1;

        if current != 0
        {
            panic!("size must be a power of 2");
        }

        Self{order}
    }

    fn rotate(&self, mut pos: Pos2<usize>, check: Pos2<usize>, value: usize) -> Pos2<usize>
    {
        if check.y != 0
        {
            return pos;
        }

        if check.x == 1
        {
            pos.x = value - 1 - pos.x;
            pos.y = value - 1 - pos.y;
        }

        Pos2{x: pos.y, y: pos.x}
    }

    #[allow(dead_code)]
    pub fn point_to_value(&self, mut pos: Pos2<usize>) -> usize
    {
        let n = 2_usize.pow(self.order as u32);

        (0..self.order).rev().map(|s|
        {
            let s = 2_usize.pow(s as u32);

            let rx = ((pos.x & s) > 0) as usize;
            let ry = ((pos.y & s) > 0) as usize;

            pos = self.rotate(pos, Pos2{x: rx, y: ry}, n);

            s * s * ((3 * rx) ^ ry)
        }).sum()
    }

    pub fn value_to_point(&self, mut value: usize) -> Pos2<usize>
    {
        let mut pos = Pos2{x: 0, y: 0};

        for s in 0..self.order
        {
            let s = 2_usize.pow(s as u32);

            let rx = (value / 2) & 1;
            let ry = (value ^ rx) & 1;

            pos = self.rotate(pos, Pos2{x: rx, y: ry}, s);

            pos.x += s * rx;
            pos.y += s * ry;

            value /= 4;
        }

        pos
    }
}

fn resave(mut image: Image, config: Config)
{
    let save_path = config.save_path.unwrap();

    image.hilbertify();

    image.save(save_path).unwrap();
}

fn main()
{
    let config = Config::parse(env::args().skip(1));

    let mut image = Image::parse(
        &config.input,
        config.width,
        Color::RGB(0, 0, 0),
        config.trim_start,
        config.trim_end
    );

    if config.unhilbertify
    {
        image.unhilbertify();
    }

    if config.save_path.is_some()
    {
        resave(image, config);
        return;
    }

    let window = DrawerWindow::new(image);

    window.wait_exit();
}

#[cfg(test)]
mod tests
{
    use super::*;

    #[test]
    fn inverse_hilbert()
    {
        let n = 512;

        let curve = HilbertCurve::new(n);

        let total = n * n;
        for i in 0..total
        {
            let point = curve.value_to_point(i);

            assert_eq!(curve.point_to_value(point), i);
        }
    }
}
