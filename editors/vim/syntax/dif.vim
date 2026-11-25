" Vim syntax file for difficulty (.dif)
if exists("b:current_syntax")
  finish
endif

syn keyword difKeyword const struct enum if else while for in break continue return
syn keyword difKeyword switch case default try or mut use pub test extern unsafe void self defer
syn keyword difReserved spawn parallel yield
syn keyword difBoolean true false
syn keyword difConstant none some ok err
syn keyword difType int i8 i16 i32 u8 u16 u32 u64 f32 f64 bool str rune
syn keyword difBuiltin print range repeat expect assert

" Type.Variant, T!E, ?T
syn match difVariant "\<\u\w*\.\u\w*\>"
syn match difTypeOp "[?!]\ze\s*\h"

" Numbers (spec 1.4)
syn match difNumber "\<\d[0-9_]*\>"
syn match difNumber "\<0x\x[0-9a-fA-F_]*\>"
syn match difNumber "\<0b[01][01_]*\>"
syn match difFloat "\<\d[0-9_]*\.\d[0-9_]*\%([eE][+-]\=\d\+\)\=\>"
syn match difFloat "\<\d[0-9_]*[eE][+-]\=\d\+\>"

" Strings
syn match difEscape contained "\\[ntr\\\"{}0]\|\\u{\x\+}"
syn region difInterp contained matchgroup=difInterpDelim start="{" end="}" contains=TOP,difInterp oneline
syn region difString start=+"+ skip=+\\"+ end=+"+ oneline contains=difEscape,difInterp

" Comments
syn keyword difTodo contained TODO FIXME XXX
syn match difComment "//.*$" contains=difTodo
syn match difDocComment "///.*$" contains=difTodo

hi def link difKeyword Keyword
hi def link difReserved Keyword
hi def link difBoolean Boolean
hi def link difConstant Constant
hi def link difType Type
hi def link difBuiltin Function
hi def link difVariant Type
hi def link difTypeOp Operator
hi def link difNumber Number
hi def link difFloat Float
hi def link difString String
hi def link difEscape SpecialChar
hi def link difInterpDelim Special
hi def link difComment Comment
hi def link difDocComment SpecialComment
hi def link difTodo Todo

let b:current_syntax = "dif"
