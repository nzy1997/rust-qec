// SPDX-License-Identifier: MPL-2.0
// Input schedule adapted from itpyi/magna tests/native/gf2_kernel.cpp,
// commit 56847478c840395735146ef1e1ccdc3d8aedc10f (Mozilla Public License 2.0).
// Compile with the pinned tests/native and core/magna/native include directories.
// This emits inputs only; Rust computes expectations with unpacked-byte oracles.
#define main magna_original_gf2_main
#include "gf2_kernel.cpp"
#undef main

void emit_row(const Row& row) {
    constexpr char hex[] = "0123456789abcdef";
    std::cout << '"';
    for(size_t j = 0; j < row.size(); j += 8) {
        unsigned byte = 0;
        for(size_t k = 0; k < 8 && j + k < row.size(); k++) byte |= unsigned(row[j + k]) << k;
        std::cout << hex[byte >> 4] << hex[byte & 15];
    }
    std::cout << '"';
}

int main() {
    // Run the unchanged original packed method first, then compare the complete
    // engine state after our input-only traversal, not just a sampled output.
    packed_tests();
    const auto original_end = random_bits;
    const auto original_checks = checks;
    random_bits.seed(2026100409);
    auto checkpoint = random_bits;
    std::cout << "{\"seed\":2026100409,\"first_word\":" << checkpoint()
              << ",\"upstream_commit\":\"56847478c840395735146ef1e1ccdc3d8aedc10f\",\"cases\":[";
    size_t count = 0;
    for(size_t n : {size_t(0), size_t(1), size_t(63), size_t(64), size_t(65), size_t(127), size_t(128), size_t(129), size_t(257)})
        for(size_t m : {size_t(0), size_t(1), size_t(3), size_t(4), size_t(7), size_t(63), size_t(64), size_t(65), size_t(137)})
            for(unsigned density : {2u, 17u}) {
                if(count++) std::cout << ',';
                Matrix a(m, Row(n));
                for(auto& row : a) row = random_row(n, density);
                if(m > 2) { a[0] = Row(n); a[m - 1] = a[m - 2]; }
                std::cout << "{\"n\":" << n << ",\"m\":" << m << ",\"density\":" << density << ",\"rows\":[";
                for(size_t j = 0; j < m; j++) { if(j) std::cout << ','; emit_row(a[j]); }
                std::cout << "],\"value\":"; emit_row(random_row(n + 3));
                std::cout << ",\"mask\":"; emit_row(random_row(m));
                std::cout << ",\"initials\":[";
                // Duplicate bounds deliberately consume fresh initial rows.
                size_t i = 0;
                for(size_t limit : {size_t(0), size_t(1), size_t(63), size_t(64), size_t(65), m, m + 91}) {
                    (void)limit;
                    if(i++) std::cout << ',';
                    emit_row(random_row(n + 7));
                }
                checkpoint = random_bits;
                std::cout << "],\"next_word\":" << checkpoint() << '}';
            }
    require(count == 162 && random_bits == original_end, "complete packed input stream changed");
    checkpoint = random_bits;
    std::cout << "],\"next_word\":" << checkpoint() << ",\"packed_cases\":" << count
              << ",\"original_packed_checks\":" << original_checks << "}\n";
}
