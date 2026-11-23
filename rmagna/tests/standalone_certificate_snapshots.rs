// SPDX-License-Identifier: MPL-2.0
// Source-bound snapshot-service fixtures from itpyi/magna
// 56847478c840395735146ef1e1ccdc3d8aedc10f, foundation.cpp and certificate_cache.hpp.
use rmagna::certificate_cache::{
    CertificateCache, CertificateCacheStatistics, CertificateSnapshot,
};
use rmagna::scalar::ExactScalar;

fn entry(key: &str, blob: &str, scalar_bytes: usize, cost: f64) -> CertificateSnapshot {
    CertificateSnapshot {
        key: key.as_bytes().to_vec(),
        blob: blob.as_bytes().to_vec(),
        probability: Some(ExactScalar::one()),
        scalar_bytes,
        saved_seconds: cost,
    }
}

#[test]
fn foundation_certificate_cache() {
    let mut cache = CertificateCache::new(7, 2);
    assert!(cache.put(entry("a", "one", 0, 1.0)));
    assert!(cache.put(entry("b", "two", 0, 2.0)));
    assert!(cache.get(b"a").is_none());
    assert!(cache.get(b"b").is_some());
    assert_eq!(cache.statistics().bytes, 4);
    assert_eq!(cache.statistics().evictions, 1);
    cache.clear();
    assert!(cache.is_empty());
    assert_eq!(cache.statistics().bytes, 0);
    println!("standalone foundation/main.certificate-cache: 2 assertions");
}

#[test]
fn byte_and_entry_limits_lru_replacement_and_owned_snapshots() {
    let mut cache = CertificateCache::new(12, 2);
    assert!(cache.put(entry("a", "abc", 1, 1.5)));
    assert!(cache.put(entry("b", "def", 1, 2.5)));
    let mut snapshot = cache.get(b"a").unwrap();
    snapshot.blob[0] = 0;
    snapshot.probability = None;
    assert!(cache.put(entry("c", "g", 0, 3.5)));
    assert!(cache.get(b"b").is_none());
    let retained = cache.get(b"a").unwrap();
    assert_eq!(retained.blob, b"abc");
    assert_eq!(retained.probability, Some(ExactScalar::one()));
    assert_eq!(cache.statistics().bytes, 7);
    assert!(cache.put(entry("a", "012345678", 1, 4.5)));
    assert_eq!(cache.len(), 1);
    assert_eq!(cache.statistics().bytes, 11);
    assert_eq!(cache.statistics().evictions, 2);
    let before = cache.statistics();
    assert!(!cache.put(entry("a", "oversized", usize::MAX, 0.0)));
    assert_eq!(cache.statistics().oversize, before.oversize + 1);
    assert_eq!(cache.statistics().bytes, before.bytes);
    assert_eq!(cache.get(b"a").unwrap().blob, b"012345678");
    assert_eq!(retained.blob, b"abc");
    assert_eq!(cache.statistics().saved_seconds, 7.5);
    cache.clear();
    assert!(cache.is_empty());
    assert_eq!(cache.statistics(), CertificateCacheStatistics::default());
    assert!(cache.get(b"a").is_none());
    assert_eq!(cache.statistics().misses, 1);
}

#[test]
fn disabled_and_oversized_replacements_preserve_lru_and_all_counters_clear() {
    for (budget, entries) in [(0, 2), (8, 0)] {
        let mut disabled = CertificateCache::new(budget, entries);
        assert!(!disabled.put(entry("a", "large", usize::MAX, 0.0)));
        assert_eq!(disabled.statistics(), CertificateCacheStatistics::default());
    }
    let mut cache = CertificateCache::new(8, 2);
    cache.put(entry("a", "one", 0, 1.0));
    cache.put(entry("b", "two", 0, 2.0));
    assert!(!cache.put(entry("a", "oversized", 0, 0.0)));
    cache.put(entry("c", "tri", 0, 3.0));
    assert!(cache.get(b"a").is_none());
    assert!(cache.get(b"b").is_some());
    assert_eq!(cache.statistics().oversize, 1);
    assert_eq!(cache.statistics().evictions, 1);
    cache.clear();
    assert_eq!(cache.statistics(), CertificateCacheStatistics::default());
}
