//! Provides an alternative implementation for `Vec::drain_filter`.
//!
//! Import `VecDrainWhereExt` to extend `Vec` with an
//! `e_drain_where` method which drains all elements where
//! a predicate indicates it. The `e_` prefix is to prevent
//! name collision/confusion as `drain_filter` might be
//! stabilized as `drain_where`. Also in difference to
//! `drain_filter` this implementation doesn't run to
//! completion when dropped, allowing stopping the draining
//! from the outside (through combinators/for loop break)
//! and is not prone to double panics/panics on drop.
#[cfg(test)]
extern crate quickcheck;

use std::{isize, ptr, mem};

pub trait VecDrainWhereExt<Item> {
    /// Drains all elements from the vector where the predicate is true.
    ///
    /// Note that dropping the iterator early will stop the process
    /// of draining. So for example if you add an combinator to the
    /// drain iterator which short circuits (e.g. `any`/`all`) this
    /// will stop draining once short circuiting is hit. So use it
    /// with care.
    ///
    /// you can use fold e.g. `any(pred)` => `fold(false, |s| )
    ///
    /// # Leak Behavior
    ///
    /// For safety reasons the length of the original vector
    /// is set to 0 while the drain iterator lives.
    ///
    /// # Panic/Drop Behavior
    ///
    /// When the iterator is dropped due to an panic in
    /// the predicate the element it panicked on is leaked
    /// but all elements which have already been decided
    /// to not be drained and such which have not yet been
    /// decided about will still be in the vector safely.
    /// I.e. if the panic also causes the vector to drop
    /// they are normally dropped if not the vector still
    /// can be normally used.
    ///
    /// # Tip: non iterator short circuiting `all`/`any`
    ///
    /// Instead of `iter.any(pred)` use
    /// `iter.fold(false, |s,i| s|pred(i))`.
    ///
    /// Instead of `iter.all(pred)` use
    /// `iter.fold(true, |s,i| s&pred(i))`.
    ///
    /// And if it is fine to not call `pred` once
    /// it's found/has show to not hold but it's
    /// still required to run the iterator to end
    /// in the normal case replace the `|` with `||`
    /// and the `&` with `&&`.
    fn e_drain_where<F>(&mut self, predicate: F)
        -> VecDrainWhere<Item, F>
        where F: FnMut(&mut Item) -> bool;
}

impl<Item> VecDrainWhereExt<Item> for Vec<Item> {
    fn e_drain_where<F>(&mut self, predicate: F)
        -> VecDrainWhere<Item, F>
        where F: FnMut(&mut Item) -> bool
    {
        let ptr = self.as_mut_ptr();
        let len = self.len();
        if len == 0 {
            let nptr = 0 as *mut _;
            return VecDrainWhere {
                pos: nptr,
                gap_pos: nptr,
                end: nptr,
                self_ref: self,
                predicate
            };
        }

        if len > isize::MAX as usize {
            panic!("can not handle more then isize::MAX elements");
        }

        // leak amplification for safety
        unsafe { self.set_len(0) }

        let end = unsafe { ptr.offset(len as isize) };

        VecDrainWhere {
            pos: ptr,
            gap_pos: ptr,
            end,
            self_ref: self,
            predicate
        }
    }
}

/// Iterator for draining a vector conditionally.
#[must_use]
#[derive(Debug)]
pub struct VecDrainWhere<'a, Item: 'a, Pred> {
    pos: *mut Item,
    gap_pos: *mut Item,
    end: *mut Item,
    predicate: Pred,
    self_ref: &'a mut Vec<Item>
}

impl<'a, I: 'a, P> Iterator for VecDrainWhere<'a, I, P>
    where P: FnMut(&mut I) -> bool
{
    type Item = I;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if self.pos.is_null() || self.pos >= self.end {
                return None;
            } else {
                unsafe {
                    let ref_to_current = &mut *self.pos;
                    self.pos = self.pos.offset(1);
                    let should_be_drained = (self.predicate)(ref_to_current);
                    if should_be_drained {
                        let item = ptr::read(ref_to_current);
                        return Some(item);
                    } else {
                        if self.gap_pos < ref_to_current {
                            ptr::copy_nonoverlapping(ref_to_current, self.gap_pos, 1);
                        }
                        self.gap_pos = self.gap_pos.offset(1);
                    }
                }
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, Some(self.self_ref.len()))
    }
}

impl<'a, I: 'a, P> Drop for VecDrainWhere<'a, I, P> {
    /// If the iterator was run to completion this will
    /// set the len to the new len after drop. I.e. it
    /// will undo the leak amplification.
    ///
    /// If the iterator is dropped before completion this
    /// will move the remaining elements to the (single)
    /// gap (still) left from draining elements and then
    /// sets the new length.
    ///
    /// If the iterator is dropped because the called
    /// predicate panicked the element it panicked on
    /// is _leaked_. This is because its simply to easy
    /// to leaf the `&mut T` value in a illegal state
    /// likely to panic drop or even behave unsafely
    /// (through it surly shouldn't behave this way).
    fn drop(&mut self) {
        let pos = self.pos as usize;
        if self.pos.is_null() {
            return
        }
        let start  = self.self_ref.as_mut_ptr() as usize;
        let end = self.end as usize;
        let gap = self.gap_pos as usize;
        let item_size: usize = mem::size_of::<I>();
        unsafe {
            let cur_len = (gap - start)/item_size;
            let rem_len = (end - pos)/item_size;
            ptr::copy(self.pos, self.gap_pos, rem_len);
            self.self_ref.set_len(cur_len + rem_len);
        }
    }
}


#[cfg(test)]
mod tests {
    use quickcheck::TestResult;
    //Uhm, this is not unused at all, so it being displayed
    // as such is a rustc bug (is in the bug tracker).
    #[allow(unused_imports)]
    use super::VecDrainWhereExt;

    mod check_with_mask {
        use super::*;

        fn cmp_with_mask(mask: Vec<bool>) -> TestResult {
            let mut data = (0..mask.len()).collect::<Vec<_>>();
            let data2 = data.clone();
            let new_len = mask.len() - mask.iter().fold(0, |s,i| if *i { s + 1 } else { s });
            let mut mask_iter = mask.clone().into_iter();
            let mut last_el: Option<usize> = None;

            let mut failed = None;
            data.e_drain_where(|el| {
                if let Some(lel) = last_el {
                    if lel + 1 != *el {
                        failed = Some(TestResult::error(
                            format!("unexpected element (exp {}, got {})", lel + 1, el)));
                    }
                }
                last_el = Some(*el);

                if let Some(mask) = mask_iter.next() {
                    mask
                } else {
                    failed = Some(TestResult::error("called predicate to often"));
                    false
                }
            }).for_each(drop);

            if let Some(f) = failed {
                return f;
            }

            if new_len != data.len() {
                return TestResult::error(format!(
                    "rem count: {}, found count: {} - {:?} | {:?}",
                    new_len, data.len(), data, mask
                ))
            }

            let expected = data2.iter().zip(mask.iter())
                    .filter(|&(_d, p)| *p)
                    .map(|(d, _p)| *d)
                    .collect::<Vec<_>>();

            if expected != data {
                TestResult::error("unexpected data");
            }
            TestResult::passed()

        }

        #[test]
        fn qc_cmp_with_mask() {
            ::quickcheck::quickcheck(cmp_with_mask as fn(Vec<bool>) -> TestResult);
        }


        #[test]
        fn fix_divide_byte_len_by_size_of() {
            let res = cmp_with_mask(vec![false]);
            assert!(!res.is_error(), "{:?}", res)
        }

        #[test]
        fn fix_update_last_el_in_test() {
            let res = cmp_with_mask(vec![false, false, false]);
            assert!(!res.is_error(), "{:?}", res)
        }
    }

    mod check_with_panic {
        use super::*;

        fn panic_situations(mask: Vec<(bool, bool)>) -> TestResult {
            let mut data = (0..mask.len()).collect::<Vec<_>>();
            let mut mask_iter = mask.clone().into_iter();
            let mut fail = None;
            let mut expect_panic = false;
            let expected_len = mask.iter()
                .fold(0, |sum, &(msk, pnk)| {
                    if expect_panic { sum + 1 }
                    else if pnk { expect_panic=true; sum }
                    else if msk { sum }
                    else { sum + 1}
                });

            let res = ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
                data.e_drain_where(|_item| {
                    let (mask, do_panic) = mask_iter.next()
                        .unwrap_or_else(|| {
                            fail = Some(TestResult::error("unexpected no more masks"));
                            (false, false)
                        });

                    if do_panic {
                        panic!("-- yes panic --");
                    }
                    mask
                }).for_each(drop);
            }));

            if let Some(failure) = fail {
                return failure;
            }

            if expect_panic {
                if res.is_ok() {
                    return TestResult::error(format!(
                        "unexpectedly no panic? exp {}, len {}, ({:?})",
                        expected_len, mask.len(), mask
                    ))
                }
            } else {
                if res.is_err() {
                    return TestResult::error(format!(
                        "unexpectedly error? exp {}, len {}, ({:?})",
                        expected_len, mask.len(), mask
                    ))
                }
            }

            if data.len() != expected_len {
                return TestResult::error(format!(
                    "unexpected resulting len {}, exp {} ({:?} - {:?})",
                    data.len(), expected_len, data, mask
                ));
            }

            TestResult::passed()
        }


        #[test]
        fn qc_panic_test() {
            ::quickcheck::quickcheck(panic_situations as fn(Vec<(bool,bool)>) -> TestResult)
        }

        #[test]
        fn fix_messed_up_test() {
            let res = panic_situations(vec![(true, false)]);
            assert!(!res.is_error(), "{:?}", res);
        }
    }

}
