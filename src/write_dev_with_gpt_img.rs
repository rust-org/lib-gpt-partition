use std::io::{Write};

trait ReadTrait {
    fn read_vec(&mut self, size: isize) -> Result<Vec<u8>, std::io::Error>;
    }

impl<R: std::io::Read> ReadTrait for R {
    fn read_vec(&mut self, size: isize) -> Result<Vec<u8>, std::io::Error> {
        let mut vec:Vec<u8> = Vec::new();
        if size < 0 {
            self.read_to_end(&mut vec)?;
            }
        else if size > 0 {
            vec = Vec::with_capacity(size as usize);
            unsafe {vec.set_len(size as usize);}
            let mut buf = &mut vec[..];
            let mut readed = 0;
            while !buf.is_empty() {
                match self.read(buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        readed += n;
                        let tmp = buf;
                        buf = &mut tmp[n..];
                        }
                    Err(e) => return Err(e),
                    }
                }
            unsafe {vec.set_len(readed);}
            }
        return Ok(vec);
        }
    }


pub fn write_dev_with_gpt_img<R: std::io::Read>(gpt_img_reader: &mut R, dev_path: &str) -> Result<(), std::io::Error> {
    let mut fdev = std::fs::File::create(dev_path)?;
    loop {
        let data = gpt_img_reader.read_vec(9 * 1024 * 1024)?;
        if data.len() == 0 {break;}
        fdev.write_all(&data)?;
        }
    fdev.flush()?;
    Ok(())
    }
