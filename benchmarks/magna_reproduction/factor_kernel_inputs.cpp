// SPDX-License-Identifier: MPL-2.0
// Input traversal adapted from itpyi/magna tests/native/gf2_kernel.cpp at
// 56847478c840395735146ef1e1ccdc3d8aedc10f (Mozilla Public License 2.0).
// Emits inputs only. All expected Rust results use independent byte oracles.
#define main magna_original_gf2_main
#include "gf2_kernel.cpp"
#undef main

namespace {
size_t draws = 0;
uint64_t fingerprint = UINT64_C(14695981039346656037);
uint64_t byte_fingerprint = UINT64_C(14695981039346656037);
uint64_t word() {
    uint64_t value = random_bits();
    ++draws;
    fingerprint = (fingerprint ^ value) * UINT64_C(1099511628211);
    for(unsigned i = 0; i < 8; ++i)
        byte_fingerprint = (byte_fingerprint ^ ((value >> (8 * i)) & 255)) * UINT64_C(1099511628211);
    return value;
}
Row input_row(size_t n, unsigned density = 2) {
    Row result(n);
    for(auto& b : result) b = word() % density == 0;
    return result;
}
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
void emit_matrix(const Matrix& rows) {
    std::cout << '[';
    for(size_t j = 0; j < rows.size(); j++) { if(j) std::cout << ','; emit_row(rows[j]); }
    std::cout << ']';
}
void emit_factor(const Matrix& a, size_t n, size_t& count) {
    if(count++) std::cout << ',';
    std::cout << "{\"n\":" << n << ",\"rows\":";
    emit_matrix(a);
    std::cout << ",\"trials\":[";
    const size_t trials = a.size() <= 5 ? size_t(1) << a.size() : 16;
    for(size_t trial = 0; trial < trials; trial++) {
        if(trial) std::cout << ',';
        Row rhs = a.size() <= 5 ? unpack(Bits(trial), a.size()) : input_row(a.size());
        if(trial % 2 && a.size() > 5) rhs = product(a, input_row(n));
        std::cout << "{\"rhs\":"; emit_row(rhs); std::cout << ",\"seed\":";
        // Exactly the original consistency branch consumes seed coordinates.
        if(consistent(a, rhs, n)) emit_row(input_row(n));
        else std::cout << "null";
        std::cout << '}';
    }
    auto checkpoint = random_bits;
    std::cout << "],\"next_word\":" << checkpoint() << '}';
}
}
int main() {
    packed_tests();
    const auto packed_end = random_bits;
    factor_tests();
    quadratic_tests();
    const auto original_end = random_bits;
    const auto original_checks = checks;
    random_bits.seed(2026100409);
    for(size_t j = 0; j < 708504; ++j) (void)word();
    require(random_bits == packed_end, "full packed-prefix engine state");
    std::cout << "{\"upstream_commit\":\"56847478c840395735146ef1e1ccdc3d8aedc10f\","
              << "\"seed\":2026100409,\"packed_draws\":" << draws << ",\"factor_cases\":[";
    size_t count = 0;
    for(size_t n = 0; n <= 5; n++)
        for(size_t m = 0; m <= 5; m++)
            for(size_t trial = 0; trial < 3; trial++) {
                Matrix a(m, Row(n));
                if(trial) for(auto& row : a) row = input_row(n);
                if(trial == 2 && m > 1) a.back() = a.front();
                emit_factor(a, n, count);
            }
    for(size_t n : {size_t(63), size_t(64), size_t(65), size_t(127), size_t(128), size_t(129), size_t(257)}) {
        for(size_t m : {size_t(0), size_t(1), size_t(7), size_t(70)}) {
            Matrix a(m, Row(n));
            for(auto& row : a) row = input_row(n);
            if(m > 2) { a.back() = a.front(); a[m - 2] = Row(n); }
            emit_factor(a, n, count);
        }
        Matrix identity(n, Row(n));
        for(size_t j = 0; j < n; j++) identity[j][j] = 1;
        emit_factor(identity, n, count);
    }
    const size_t factor_count = count;
    count = 0;
    std::cout << "],\"quadratic_cases\":[";
    const std::vector<std::pair<size_t, size_t>> shapes = {{0, 0}, {0, 5}, {1, 0}, {1, 1}, {3, 5}, {5, 3}, {7, 7},
        {63, 6}, {64, 7}, {65, 8}, {65, 65}, {129, 67}, {137, 129}};
    for(const auto& [n, k] : shapes)
        for(unsigned density : {2u, 11u}) {
            if(count++) std::cout << ',';
            std::vector<int> linear(n);
            Matrix upper(n, Row(n)), basis(k, Row(n));
            for(size_t j = 0; j < n; j++) {
                linear[j] = int(word() % 4);
                for(size_t l = j + 1; l < n; l++) upper[j][l] = word() % density == 0;
            }
            for(auto& row : basis) row = input_row(n, density);
            if(k > 2) { basis[0] = Row(n); basis[k - 1] = basis[k - 2]; }
            auto shift = input_row(n);
            std::cout << "{\"n\":" << n << ",\"k\":" << k << ",\"density\":" << density << ",\"linear\":[";
            for(size_t j = 0; j < n; j++) { if(j) std::cout << ','; std::cout << linear[j]; }
            std::cout << "],\"upper\":"; emit_matrix(upper);
            std::cout << ",\"basis\":"; emit_matrix(basis);
            std::cout << ",\"shift\":"; emit_row(shift);
            std::cout << ",\"samples\":[";
            const size_t samples = k <= 8 ? size_t(1) << k : 64;
            for(size_t sample = 0; sample < samples; sample++) {
                if(sample) std::cout << ',';
                emit_row(k <= 8 ? unpack(Bits(sample), k) : input_row(k));
            }
            auto checkpoint = random_bits;
            std::cout << "],\"next_word\":" << checkpoint() << '}';
        }
    require(factor_count == 143 && count == 26, "complete original remaining case schedule");
    require(random_bits == original_end, "full original input traversal engine state");
    std::cout << "],\"total_draws\":" << draws << ",\"fnv64\":" << fingerprint
              << ",\"byte_fnv64\":" << byte_fingerprint << ",\"original_checks\":" << original_checks << "}\n";
}
