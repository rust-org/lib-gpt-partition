use gpt;
use std::{os::unix::fs::FileTypeExt, io::{Seek}};

#[derive(Debug)]
pub struct GptPartitionCursor {
    pub file: std::fs::File,
    partition_offset: u64,
    pub size: u64,
    pub pos: u64,
    pub part_num: u32,
    }

impl GptPartitionCursor {
    pub fn new(path: &str, partition_name: &str) -> Result<Self, std::io::Error> {
        let metadata = std::fs::metadata(path)?;
        if ! metadata.file_type().is_block_device(){
            eprintln!("Warning: {} is not a block device.", path);
            // Err(Err(io::Error::new(
            //         io::ErrorKind::InvalidInput,
            //         format!("{path} is not a block device.", path)
            //         )))
            }

        let disk = gpt::GptConfig::new().writable(false).open(path)?;
        let partitions = disk.partitions();

        let mut partition: Option<&gpt::partition::Partition> = None;
        let mut part_num = 0u32;
        for (i, p) in partitions {
            if partition_name.eq_ignore_ascii_case(&p.name){
                partition = Some(p);
                part_num = i.clone();
                break;
                }
            }

        if partition == None {
            if let Ok(i) = partition_name.parse::<u32>() {
                partition = partitions.get(&i);
                }
            if partition == None {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("Failed to find specified partition {} in GPT", partition_name),
                    ));
                }
            }

        let info = partition.unwrap();
        let file = std::fs::OpenOptions::new()
                        .read(true)
                        .write(true)
                        .open(path)?;
        let block_size = *disk.logical_block_size(); // 512
        let partition_offset = info.bytes_start(block_size).unwrap();

        // file.seek(std::io::SeekFrom::Start(partition_offset))?;

        Ok(Self {
            file,
            partition_offset,
            size: info.bytes_len(block_size).unwrap(),
            pos: 0,
            part_num
            })
        }

    pub fn part_num(&self) -> u32{
        return self.part_num;
        }
    }


impl std::io::Read for GptPartitionCursor {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let bytes_to_read = std::cmp::min(buf.len() as u64, self.size - self.pos);
        if bytes_to_read == 0 {
            return Ok(0);
            }
        self.file.seek(std::io::SeekFrom::Start(self.partition_offset + self.pos))?;
        let bytes_read = self.file.read(&mut buf[..bytes_to_read as usize])?;
        self.pos += bytes_read as u64;
        Ok(bytes_read)
        }
    }

impl std::io::Write for GptPartitionCursor {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let bytes_to_write = std::cmp::min(buf.len() as u64, self.size - self.pos);
        if bytes_to_write == 0 {
            return Ok(0);
            }

        self.file.seek(std::io::SeekFrom::Start(self.partition_offset + self.pos))?;
        let bytes_written = self.file.write(&buf[..bytes_to_write as usize])?;
        self.pos += bytes_written as u64;
        Ok(bytes_written)
        }

    fn flush(&mut self) -> std::io::Result<()> {
        self.file.flush()
        }
    }


impl std::io::Seek for GptPartitionCursor {
    fn seek(&mut self, pos: std::io::SeekFrom) -> std::io::Result<u64> {
        match pos {
            std::io::SeekFrom::Start(new_pos) => {
                if new_pos <= self.size as u64 {
                    self.pos = new_pos;
                    Ok(new_pos)
                    }
                else{
                    Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        "Cannot seek past end of data",
                        ))
                    }
                }
            std::io::SeekFrom::End(offset) => {
                if self.pos as i64 + offset <= self.size as i64 {
                    self.pos = (self.size as i64 + offset) as u64;
                    Ok((self.pos) as u64)
                    }
                else{
                    Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        "Cannot seek past end of data",
                        ))
                    }
                }
            std::io::SeekFrom::Current(offset) => {
                let new_pos = (self.pos as i64 + offset) as usize;
                if new_pos <= self.size as usize {
                    self.pos = new_pos as u64;
                    Ok((self.pos) as u64)
                    }
                else{
                    Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        "Cannot seek past end of data",
                        ))
                    }
                }
            }
        }
    }
