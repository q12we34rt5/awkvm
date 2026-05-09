// libc number parsing. atoi / atol / atoll for ints; atof / strtod
// for floats; strtoX accept (and ignore) the endptr / base args
// because the dominant call shape is `strtol(s, NULL, 10)`. C
// semantics: stop at the first non-numeric character (the
// "trailing" case below).
#include <stdio.h>
#include <stdlib.h>

int main(void) {
    printf("atoi=%d atol=%ld atoll=%lld\n",
           atoi("42"), atol("-12345"), atoll("9876543210"));
    printf("strtol=%ld\n", strtol("100", NULL, 10));
    printf("atof=%.2f strtod=%.4f\n",
           atof("3.14"), strtod("-2.5e3", NULL));
    printf("trailing=%d\n", atoi("99abc"));
    return 0;
}
