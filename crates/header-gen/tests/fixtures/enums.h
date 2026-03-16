/* enums.h — C enum used as a struct field */
#ifndef ENUMS_H
#define ENUMS_H

typedef enum Color {
    COLOR_RED   = 0,
    COLOR_GREEN = 1,
    COLOR_BLUE  = 2,
} Color;

typedef struct Pixel {
    int   x;
    int   y;
    Color color;
} Pixel;

#endif /* ENUMS_H */
