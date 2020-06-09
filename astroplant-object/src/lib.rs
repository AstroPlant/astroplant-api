use bytes::Bytes;
use futures::sink::SinkExt;
use futures::stream::{Stream, StreamExt};
use rusoto_core::region::Region as S3Region;
use rusoto_s3::S3;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use tokio::io::AsyncReadExt;

#[derive(Debug)]
pub enum Error {
    S3Put(rusoto_core::RusotoError<rusoto_s3::PutObjectError>),
    S3Get(rusoto_core::RusotoError<rusoto_s3::GetObjectError>),
    S3NoFile,
    LocalIo(tokio::io::Error),
    LocalOther,
}
pub type Result<T> = std::result::Result<T, Error>;

#[derive(Clone)]
enum Stores {
    S3 {
        s3: rusoto_s3::S3Client,
        bucket_name: String,
    },
    Local {
        root: PathBuf,
    },
}

// Ideally this would be a trait, but we can't yet "just" use async functions in traits.
#[derive(Clone)]
pub struct ObjectStore {
    store: Stores,
}

impl ObjectStore {
    pub fn s3(region_name: String, region_endpoint: String) -> Self {
        let s3 = rusoto_s3::S3Client::new(S3Region::Custom {
            name: region_name,
            endpoint: region_endpoint,
        });

        Self {
            store: Stores::S3 {
                s3,
                bucket_name: "kit-files".to_owned(),
            },
        }
    }

    pub fn local(root: &str) -> Self {
        Self {
            store: Stores::Local {
                root: PathBuf::from(root),
            },
        }
    }

    pub async fn put(
        &self,
        kit_serial: &str,
        object_name: &str,
        data: Vec<u8>,
        media_type: String,
    ) -> Result<()> {
        let key = format!("{}/{}", kit_serial, object_name);
        match &self.store {
            Stores::S3 { s3, bucket_name } => {
                let mut request = rusoto_s3::PutObjectRequest::default();
                request.bucket = bucket_name.clone();
                request.content_type = Some(media_type);
                request.key = key;
                request.body = Some(data.into());
                s3.put_object(request).await.map_err(Error::S3Put)?;
                Ok(())
            }
            Stores::Local { root } => {
                let path = root.join(Path::new(&key));
                tokio::fs::create_dir_all(path.parent().ok_or(Error::LocalOther)?)
                    .await
                    .map_err(Error::LocalIo);
                tokio::fs::write(path, &data).await.map_err(Error::LocalIo)
            }
        }
    }

    pub async fn get(
        &self,
        kit_serial: &str,
        object_name: &str,
    ) -> Result<
        Pin<Box<dyn Stream<Item = std::result::Result<Bytes, std::io::Error>> + Send + 'static>>,
    > {
        let key = format!("{}/{}", kit_serial, object_name);
        match &self.store {
            Stores::S3 { s3, bucket_name } => {
                let mut request = rusoto_s3::GetObjectRequest::default();
                request.bucket = bucket_name.clone();
                request.key = key;
                let output = s3.get_object(request).await.map_err(Error::S3Get)?;
                match output.body {
                    Some(bytes_stream) => Ok(bytes_stream.boxed()),
                    None => Err(Error::S3NoFile),
                }
            }
            Stores::Local { root } => {
                let (mut tx, rx) = futures::channel::mpsc::channel(16);

                let path = root.join(Path::new(&key));
                let mut file = tokio::fs::File::open(path).await.map_err(Error::LocalIo)?;

                // Spawn a task to read file contents into the sink-part of the channel.
                tokio::spawn(async move {
                    let mut buffer = [0; 1024 * 32];
                    loop {
                        let result = match file.read(&mut buffer[..]).await {
                            Ok(n) => {
                                println!("buffer size: {}", n);
                                if n == 0 {
                                    break;
                                } else {
                                    tx.send(Ok(bytes::Bytes::copy_from_slice(&buffer[..n])))
                                        .await
                                }
                            }
                            Err(err) => tx.send(Err(err)).await,
                        };
                        if result.is_err() {
                            break;
                        }
                    }
                });

                Ok(rx.boxed())
            }
        }
    }
}
