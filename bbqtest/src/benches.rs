use criterion::{black_box, criterion_group, criterion_main, Criterion};
use bbqueue::{BBBuffer, consts::*, Producer, Consumer, ArrayLength};
use std::thread::spawn;
use std::cmp::min;
use std::sync::Arc;

const DATA_SZ: usize = 128 * 1024 * 1024;

pub fn criterion_benchmark(c: &mut Criterion) {

    let mut data = vec![0; DATA_SZ].into_boxed_slice();

    // c.bench_function("bbq 128", |bench| {
    //     bench.iter(|| {
    //         chunky(
    //             &data,
    //             128
    //         )
    //     })
    // });

    // c.bench_function("bbq 256", |bench| {
    //     bench.iter(|| {
    //         chunky(
    //             &data,
    //             256
    //         )
    //     })
    // });

    // c.bench_function("bbq 512", |bench| {
    //     bench.iter(|| {
    //         chunky(
    //             &data,
    //             512
    //         )
    //     })
    // });

    // c.bench_function("bbq 1024", |bench| {
    //     bench.iter(|| {
    //         chunky(
    //             &data,
    //             1024
    //         )
    //     })
    // });

    c.bench_function("bbq 2048", |bench| {
        bench.iter(|| {
            chunky(
                &data,
                2048
            )
        })
    });

    let buffy: BBBuffer<U65536> = BBBuffer::new();
    let (mut prod, mut cons) = buffy.try_split().unwrap();


    c.bench_function("bbq 2048-2", |bench| {

        let chunksz = 8192;

        bench.iter(|| { black_box(
            thread::scope(|sc| {
                sc.spawn(|_| {
                    data.chunks(chunksz).for_each(|ch| {
                        loop {
                            if let Ok(mut wgr) = prod.grant_exact(chunksz) {
                                wgr.copy_from_slice(black_box(ch));
                                wgr.commit(chunksz);
                                break;
                            }
                        }
                    });
                });

                sc.spawn(|_| {
                    data.chunks(chunksz).for_each(|ch| {
                        let mut st = 0;
                        loop {
                            if let Ok(rgr) = cons.read() {
                                let len = min(chunksz - st, rgr.len());
                                assert_eq!(ch[st..st+len], rgr[..len]);
                                rgr.release(len);

                                st += len;

                                if st == chunksz {
                                    break;
                                }
                            }
                        }
                    });
                });
            })).unwrap();
        })
    });

    use std::mem::MaybeUninit;


    c.bench_function("std channels", |bench| {

        bench.iter(|| {
            use std::sync::mpsc::{Sender, Receiver};
            let (mut prod, mut cons): (Sender<[u8; 8192]>, Receiver<[u8; 8192]>) = std::sync::mpsc::channel();
            let rdata = &data;

            thread::scope(|sc| {
                sc.spawn(move |_| {
                    rdata.chunks(8192).for_each(|ch| {
                        let mut x: MaybeUninit<[u8; 8192]> = MaybeUninit::uninit();
                        unsafe {
                            x.as_mut_ptr().copy_from_nonoverlapping(ch.as_ptr().cast::<[u8; 8192]>(), 1)
                        };
                        prod.send(unsafe { x.assume_init() }).unwrap();
                    });
                });

                sc.spawn(move |_| {
                    rdata.chunks(8192).for_each(|ch| {
                        let x = cons.recv().unwrap();
                        assert_eq!(&x[..], &ch[..]);
                    });
                });
            }).unwrap();
        })
    });

    c.bench_function("xbeam channels", |bench| {

        bench.iter(|| {
            use crossbeam::{bounded, Sender, Receiver};
            let (mut prod, mut cons): (Sender<[u8; 8192]>, Receiver<[u8; 8192]>) = bounded(65536 / 8192);
            let rdata = &data;

            thread::scope(|sc| {
                sc.spawn(move |_| {
                    rdata.chunks(8192).for_each(|ch| {
                        let mut x: MaybeUninit<[u8; 8192]> = MaybeUninit::uninit();
                        unsafe {
                            x.as_mut_ptr().copy_from_nonoverlapping(ch.as_ptr().cast::<[u8; 8192]>(), 1)
                        };
                        prod.send(unsafe { x.assume_init() }).unwrap();
                    });
                });

                sc.spawn(move |_| {
                    rdata.chunks(8192).for_each(|ch| {
                        let x = cons.recv().unwrap();
                        assert_eq!(&x[..], &ch[..]);
                    });
                });
            }).unwrap();
        })
    });
}

use crossbeam_utils::thread;
fn chunky(data: &[u8], chunksz: usize) {
    let buffy: BBBuffer<U4096> = BBBuffer::new();
    let (mut prod, mut cons) = buffy.try_split().unwrap();

    thread::scope(|sc| {
        let pjh = sc.spawn(|_| {
            data.chunks(chunksz).for_each(|ch| {
                loop {
                    if let Ok(mut wgr) = prod.grant_exact(chunksz) {
                        wgr.copy_from_slice(ch);
                        wgr.commit(chunksz);
                        break;
                    }
                }
            });
        });

        let cjh = sc.spawn(|_| {
            data.chunks(chunksz).for_each(|ch| {
                let mut st = 0;
                loop {
                    if let Ok(rgr) = cons.read() {
                        let len = min(chunksz - st, rgr.len());
                        assert_eq!(ch[st..st+len], rgr[..len]);
                        rgr.release(len);

                        st += len;

                        if st == chunksz {
                            break;
                        }
                    }
                }
            });
        });
    }).unwrap();
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);