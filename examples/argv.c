#include <stdio.h>
#include <string.h>

int main(int argc, char **argv) {
    printf("argc=%d\n", argc);
    for (int i = 0; i < argc; i++) {
        printf("argv[%d]=%s (len=%lu)\n", i, argv[i], strlen(argv[i]));
    }
    // Sum of strlen of args 1..n-1
    int total = 0;
    for (int i = 1; i < argc; i++) total += strlen(argv[i]);
    return total;
}
