//!
//! # Magic Map
//!
//!
//!
//!

use mmap_sync::guard::ReadResult;
use mmap_sync::synchronizer::Synchronizer;
use rkyv::ser::serializers::AllocSerializer;
use rkyv::Archive;
use rkyv::Archived;
use rkyv::Deserialize;
use rkyv::Serialize;
use std::ops::Deref;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;
use thiserror::Error;

///
/// # MagicMap
///
/// MagicMap is a wrapper around the [`Synchronizer`] that provides APIs to read and write data in
/// a concurrent environment. It also uses [`T`] to and a type inference/type level check to ensure
/// that the data that is being read and written is of the same type.
///
/// The type also implements [`Clone`] to make it easier to share the same instance across multiple
/// threads/contexts. The [`Clone`] implementation however does not clone the underlying data, but
/// the [`Arc`] that holds the [`Synchronizer`] instance. So, this map doesn't need to be wrapped
/// in [`Arc`].
///
///
pub struct MagicMap<T: Archive> {
    writer: Arc<Mutex<Synchronizer>>,
    reader: Arc<Synchronizer>,
    _phantom: std::marker::PhantomData<T>,
}

impl<T: Archive> Clone for MagicMap<T> {
    fn clone(&self) -> Self {
        Self {
            writer: self.writer.clone(),
            reader: self.reader.clone(),
            _phantom: self._phantom,
        }
    }
}

impl<T: Archive> MagicMap<T> {
    ///
    /// Creating a instance of [`MagicMap`] with a file path. The file path is then mapped to
    /// memory and used to read and write data. The file is created if it doesn't exist.
    ///
    pub fn new(file: PathBuf) -> Self {
        let writer = Arc::new(Mutex::new(Synchronizer::new(file.as_ref())));
        let reader = Arc::new(Synchronizer::new(file.as_ref()));
        Self {
            writer,
            reader,
            _phantom: std::marker::PhantomData,
        }
    }

    ///
    /// Exposes a pure write operation. It takes a data of type [`T`] and writes it
    /// to the underlying file. This will overwrite the existing data in the file.
    ///
    /// This uses the `mmap` syscall to sync the file to persistant storage which reduces the overhead of the
    /// operation while still keeping the write lightweight
    ///
    pub fn create(self, data: T) -> Result<(), MagicMapError>
    where
        for<'b> <T as rkyv::Archive>::Archived:
            rkyv::CheckBytes<rkyv::validation::validators::DefaultValidator<'b>>,
        T: Serialize<AllocSerializer<1_000_000>>,
    {
        let writer = self.writer.lock();
        match writer {
            Ok(mut guard) => {
                let output = guard.write(&data, Duration::from_secs(5));

                output.map(|_| ()).map_err(|_| MagicMapError::WriteFailure)
            }
            Err(_e) => Err(MagicMapError::LockWriterError),
        }
    }

    ///
    /// This function exposes a update API. It allows you to mutate the state of the data by
    /// providing you a owned instance of the data and allowing you to edit it in a [`Result`]
    /// context. If the operation is successful, the data is written back to the file.
    ///
    /// There exists a fallback mechanism that allows you to provide a function ([`impl FnOnce() -> Result<T, MagicMapError>`]) that will be called
    /// if the read operation fails. This is useful when you want to provide a default value that
    /// is to be written to the file. When not required, you can pass an empty tuple [`()`].
    ///
    pub fn write(
        self,
        execution: impl FnOnce(T) -> Result<T, MagicMapError>,
        fallback: impl Fallback<T>,
    ) -> Result<(), MagicMapError>
    where
        for<'b> <T as rkyv::Archive>::Archived:
            rkyv::CheckBytes<rkyv::validation::validators::DefaultValidator<'b>>,
        Archived<T>: Deserialize<T, rkyv::Infallible>,
        T: Serialize<AllocSerializer<1_000_000>>,
    {
        let writer = self.writer.lock();
        match writer {
            Ok(mut guard) => {
                let failable_data =
                    unsafe { guard.read::<T>(true) }.map_err(|_| MagicMapError::ReadDataError);

                let updated = failable_data
                    .and_then(|data| {
                        let inner_data: &Archived<T> = data.deref();

                        let output: T = inner_data
                            .deserialize(&mut rkyv::Infallible)
                            .map_err(|_| MagicMapError::DeserializationError)?;

                        drop(data);

                        execution(output)
                    })
                    .or_else(|e| {
                        eprintln!("Failed to read data: {:?}", e);
                        fallback.fallback()
                    })?;

                let output = guard.write(&updated, Duration::from_secs(5));

                output
                    .map(|_| ())
                    .map_err(|_| MagicMapError::LockWriterError)
            }
            Err(_e) => Err(MagicMapError::LockWriterError),
        }
    }

    ///
    /// This function exposes a read operation. It allows you to read the data from the memory
    /// mapped file and provide you with a [`ReadResult`] that you can use to read the data.
    ///
    /// This happens in a closure where the `ReadGuard` exists allowing you to perform extraction of
    /// whatever data you want from the memory mapped file. You can then return the data in the
    /// closure and it will be returned by the function
    ///
    pub fn read<U>(
        self,
        execution: impl FnOnce(ReadResult<'_, T>) -> Result<U, MagicMapError>,
    ) -> Result<U, MagicMapError>
    where
        for<'b> <T as rkyv::Archive>::Archived:
            rkyv::CheckBytes<rkyv::validation::validators::DefaultValidator<'b>>,
    {
        let reader = Arc::into_raw(self.reader);
        let output = unsafe {
            let sync = &mut *(reader as *mut Synchronizer);
            let data = sync.read::<T>(true);

            data
        }
        .map_err(|_| MagicMapError::ReadDataError)?;

        execution(output)
    }

    ///
    /// Similar to the [`read`] function, this function clones the data from the memory mapped file
    /// and returns it as an owned instance of the data.
    ///
    pub fn get_owned(self) -> Result<T, MagicMapError>
    where
        for<'b> <T as rkyv::Archive>::Archived:
            rkyv::CheckBytes<rkyv::validation::validators::DefaultValidator<'b>>,
        Archived<T>: Deserialize<T, rkyv::Infallible>,
    {
        self.read(|data| {
            let data: &Archived<T> = data.deref();
            let output: T = data
                .deserialize(&mut rkyv::Infallible)
                .map_err(|_| MagicMapError::DeserializationError)?;

            Ok(output)
        })
    }
}

#[derive(Error, Debug, Clone)]
pub enum MagicMapError {
    #[error("Failed to lock writer")]
    LockWriterError,
    #[error("Failed to read data")]
    ReadDataError,
    #[error("Failed to write via synchronizer")]
    WriteFailure,
    #[error("Failed while deserializing memory")]
    DeserializationError,
}

pub trait Fallback<T> {
    fn fallback(self) -> Result<T, MagicMapError>;
}

impl<T, F> Fallback<T> for F
where
    F: FnOnce() -> Result<T, MagicMapError>,
{
    fn fallback(self) -> Result<T, MagicMapError> {
        self()
    }
}

impl<T> Fallback<T> for () {
    fn fallback(self) -> Result<T, MagicMapError> {
        Err(MagicMapError::LockWriterError)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use rkyv::CheckBytes;

    use super::*;

    const FILE_NAME: &str = "/tmp/test_magicmap";

    #[derive(Archive, Debug, Deserialize, Serialize)]
    #[archive_attr(derive(CheckBytes))]
    struct TestStructSimple {
        a: u32,
        b: u32,
    }

    #[derive(Archive, Debug, Deserialize, Serialize)]
    #[archive_attr(derive(CheckBytes))]
    struct TestStructComplex {
        a: u32,
        b: HashMap<String, u32>,
    }

    #[test]
    fn test_simple_seq_create_read() {
        let ada = MagicMap::<TestStructSimple>::new(PathBuf::from(format!("{}_sr", FILE_NAME)));

        // add data { a: 2, b: 6 }
        let data = TestStructSimple { a: 2, b: 6 };
        ada.clone().create(data).expect("Failed to create data");

        // asset a and b in seperate calls
        let a = ada.read(|data| Ok(data.a)).expect("Failed to read data");

        assert_eq!(a, 2);
    }

    #[test]
    fn test_simple_seq_create_rwr() {
        let ada = MagicMap::<TestStructSimple>::new(PathBuf::from(format!("{}_srw", FILE_NAME)));

        // add data { a: 2, b: 6 }
        let data = TestStructSimple { a: 2, b: 6 };
        ada.clone().create(data).expect("Failed to create data");

        // asset a and b in seperate calls
        let a = ada
            .clone()
            .read(|data| Ok(data.a))
            .expect("Failed to read data");

        assert_eq!(a, 2);

        ada.clone()
            .write(
                |mut data| {
                    data.a = 3;
                    Ok(data)
                },
                (),
            )
            .expect("Failed to write data");

        let a = ada
            .clone()
            .read(|data| Ok(data.a))
            .expect("Failed to read data");

        assert_eq!(a, 3);
    }

    #[test]
    fn test_simple_con_create_r() {
        let ada = MagicMap::<TestStructSimple>::new(PathBuf::from(format!("{}_r", FILE_NAME)));

        // add data { a: 2, b: 6 }
        let data = TestStructSimple { a: 2, b: 6 };
        ada.clone().create(data).expect("Failed to create data");

        let (tx, rx) = std::sync::mpsc::channel::<()>();

        let ada_clone = ada.clone();

        let handle = std::thread::spawn(move || {
            let mut output = vec![];
            loop {
                let data = rx.try_recv();

                if data.is_ok() {
                    break;
                }

                let a = ada_clone
                    .clone()
                    .read(|data| Ok(data.a))
                    .expect("Failed to read data");

                output.push(a);
                std::thread::sleep(Duration::from_micros(1));
            }
            output
        });
        std::thread::sleep(Duration::from_micros(1));
        ada.clone()
            .write(
                |mut value| {
                    value.a = 3;
                    Ok(value)
                },
                (),
            )
            .expect("Failed to write data");
        std::thread::sleep(Duration::from_micros(1));

        tx.send(()).expect("Failed to send signal");

        let output = handle.join().expect("Failed to join thread");

        let current_state = ada
            .clone()
            .read(|data| Ok(data.a))
            .expect("Failed to read data");

        let final_state = output
            .iter()
            .filter_map(|data| {
                if *data == 2 || *data == 3 {
                    None
                } else {
                    Some(*data)
                }
            })
            .collect::<Vec<_>>();

        assert_eq!(final_state, Vec::<u32>::new());
        assert_eq!(current_state, 3);
    }

    #[test]
    fn test_simple_con_create_rw() {
        use rand::seq::SliceRandom;

        let ada = MagicMap::<TestStructSimple>::new(PathBuf::from(format!("{}_rw", FILE_NAME)));

        // add data { a: 2, b: 6 }
        let data = TestStructSimple { a: 2, b: 6 };
        ada.clone().create(data).expect("Failed to create data");

        let (tx, rx) = std::sync::mpsc::channel::<()>();

        let ada_clone1 = ada.clone();
        let ada_clone2 = ada.clone();

        let numbers = vec![3, 4, 5, 6, 7, 8, 9, 10];
        let my_numbers = numbers.clone();

        let write_handle = std::thread::spawn(move || {
            let mut rng = rand::thread_rng();

            let write_clone = ada_clone1.clone();

            for _ in 0..1_000 {
                let choice = *my_numbers.choose(&mut rng).unwrap();
                write_clone
                    .clone()
                    .write(
                        |mut value| {
                            value.a = choice;
                            Ok(value)
                        },
                        (),
                    )
                    .expect("Failed to write data");
            }

            tx.send(()).expect("Failed to send signal");
        });

        let read_handle = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_secs(1));
            let mut output = vec![];
            loop {
                let data = rx.try_recv();

                if data.is_ok() {
                    break;
                }

                let a = ada_clone2
                    .clone()
                    .read(|data| Ok(data.a))
                    .expect("Failed to read data");

                output.push(a);
            }
            output
        });

        write_handle.join().expect("Failed to join write thread");
        let output = read_handle.join().expect("Failed to join read thread");

        let current_state = ada
            .clone()
            .read(|data| Ok(data.a))
            .expect("Failed to read data");

        let final_state = output
            .iter()
            .filter(|data| !numbers.clone().iter().any(|x| *x == **data))
            .collect::<Vec<_>>();

        assert_eq!(final_state, Vec::<&u32>::new());
        assert!(numbers.iter().any(|x| *x == current_state));
    }

    // - concurrent writes
    // - moving underlying files
}
