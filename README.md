# vec-drain-where

A alternate `Vec::drain_filter` implementation, slightly diverging from
the std's implementation.

This crate provides an extension trait adding a `e_drain_where` method
(the `e_` prefix is used to prevent name collisions with std as the
currently `drain_filter` might be stabilized as `drain_where`).

**`e_drain_where` as one large difference to drain filter. It doesn't
run to completion when dropped and can as such be "early stopped" from
the outside by stopping the iteration an dropping the iterator.**

The reason why `Vec::drain_filter` does run the drain to completion
on drop is that it can be quite confusing. E.g. the code:

```
vec.drain_filter(|x|x.should_be_removed()).any(|x|x.had_fatal_error())
```

Would not necessary do what it's expected to do, i.e. it would drain
thinks until it finds any drained value with `had_fatal_error() == true`
_and then would stop draining_ (with `e_drain_where`).

But running to completion on drop is also tricky/dangerous e.g. it can
lead easily to thinks like panic's on drop and as such double panics,
while drop on panic behavior for `Vec::drain_filter` might still change
before stabilization, this crate completely avoids the problem at cost
of making it easy to accidentally stop the draining to early.


## Documentation

Documentation can be [viewed on docs.rs](https://docs.rs/mail-api). (at least once it's published ;=) )


## License

Licensed under either of

- Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any
additional terms or conditions.