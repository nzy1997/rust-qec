# rbposd LDPC MVP Reference

This crate provides the minimum public contract for an LDPC decoding MVP.
The initial scope intentionally locks configuration defaults, channel model
shape, and decode error variants so downstream crates can build against a
stable interface while implementation details evolve.
