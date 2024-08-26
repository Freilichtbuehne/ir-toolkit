use log::error;
use tokio::io::{AsyncBufReadExt, AsyncRead, BufReader};

pub async fn print_stream<R: AsyncRead + Unpin>(stream: Option<R>) {
    if let Some(stream) = stream {
        let mut reader = BufReader::new(stream);
        let mut buffer = vec![];

        loop {
            buffer.clear();
            match reader.read_until(b'\n', &mut buffer).await {
                Ok(0) => break, // EOF reached
                Ok(_) => {
                    // The buffer may not be a valid UTF-8 sequence
                    print!("{}", String::from_utf8_lossy(&buffer));
                }
                Err(e) => {
                    error!("Error reading stream: {}", e);
                    break;
                }
            }
        }
    }
}

pub async fn read_stream<R: AsyncRead + Unpin>(stream: Option<R>, print: bool) -> String {
    if let Some(stream) = stream {
        let mut reader = BufReader::new(stream);
        let mut buffer = vec![];
        let mut output = String::new();

        loop {
            buffer.clear();
            match reader.read_until(b'\n', &mut buffer).await {
                Ok(0) => break, // EOF reached
                Ok(_) => {
                    // The buffer may not be a valid UTF-8 sequence
                    let str = String::from_utf8_lossy(&buffer);
                    if print {
                        print!("{}", str);
                    }
                    output.push_str(&str);
                }
                Err(e) => {
                    error!("Error reading stream: {}", e);
                    break;
                }
            }
        }

        output
    } else {
        String::new()
    }
}
