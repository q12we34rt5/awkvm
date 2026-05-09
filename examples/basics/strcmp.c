// strcmp: byte-by-byte compare returning the difference of the
// first differing pair (or 0 if both reach NUL together). Each
// `printf` below asserts the SIGN of the result, which is the
// only thing C guarantees — magnitudes can vary across libc
// implementations.
#include <stdio.h>
#include <string.h>

int main(void) {
    printf("eq=%d\n",     strcmp("hello", "hello")  == 0);
    printf("lt=%d\n",     strcmp("apple", "banana") <  0);
    printf("gt=%d\n",     strcmp("zebra", "yak")    >  0);
    printf("prefix=%d\n", strcmp("foo",   "foobar") <  0);
    printf("empty=%d\n",  strcmp("",      "")       == 0);
    return 0;
}
