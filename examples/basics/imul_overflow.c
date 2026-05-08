// `mul i32` overflows i32 by up to 32 bits, so awkvm has to truncate
// the product back to the IR width. Without truncation, downstream
// bitwise normalizers (_xor, _shl, ...) see out-of-range values and
// either fatal in gawk or silently produce wrong results — the bug
// that broke mt19937 seeding (chained `* 0x9e3779b9` mixers).
#include <stdint.h>
#include <stdio.h>

int main(void) {
    volatile uint32_t a = 100000u;
    volatile uint32_t b = 100000u;
    // 10^10 mod 2^32 == 1410065408
    uint32_t product = a * b;
    // Force a downstream bitwise op so a non-truncated product would
    // diverge from the expected value.
    uint32_t mixed = product ^ 0xfffu;  // 1410065408 ^ 4095 == 1410067455
    printf("%u %u\n", product, mixed);
    return 0;
}
