;;; dif-mode.el --- Major mode for difficulty (.dif)  -*- lexical-binding: t; -*-

;;; Commentary:
;; Font-lock, 4-space indentation and `//' comments for difficulty.
;; Install: put this file on `load-path' and (require 'dif-mode).

;;; Code:

(defconst dif-keywords
  '("const" "struct" "enum" "if" "else" "while" "for" "in" "break" "continue"
    "return" "switch" "case" "default" "try" "or" "mut" "use" "pub" "test"
    "extern" "unsafe" "void" "self" "defer" "spawn" "parallel" "yield"))

(defconst dif-constants '("true" "false" "none" "some" "ok" "err"))

(defconst dif-types
  '("int" "i8" "i16" "i32" "u8" "u16" "u32" "u64" "f32" "f64" "bool" "str" "rune"))

(defconst dif-builtins '("print" "range" "repeat" "expect" "assert"))

(defconst dif-font-lock-keywords
  `((,(regexp-opt dif-keywords 'symbols) . font-lock-keyword-face)
    (,(regexp-opt dif-constants 'symbols) . font-lock-constant-face)
    (,(regexp-opt dif-types 'symbols) . font-lock-type-face)
    (,(regexp-opt dif-builtins 'symbols) . font-lock-builtin-face)
    ;; Type.Variant
    ("\\_<\\([A-Z][A-Za-z0-9_]*\\)\\.\\([A-Z][A-Za-z0-9_]*\\)\\_>"
     (1 font-lock-type-face) (2 font-lock-constant-face))
    ;; ?T and T!E
    ("[?!]\\(?:[ \t]*\\)\\([A-Za-z_][A-Za-z0-9_]*\\)" 1 font-lock-type-face)
    ;; numbers
    ("\\_<0[xX][0-9a-fA-F_]+\\_>" . font-lock-constant-face)
    ("\\_<0[bB][01_]+\\_>" . font-lock-constant-face)
    ("\\_<[0-9][0-9_]*\\(?:\\.[0-9][0-9_]*\\)?\\(?:[eE][+-]?[0-9]+\\)?\\_>"
     . font-lock-constant-face)
    ;; escapes and {interpolation} inside strings
    ("\\\\\\(?:[ntr\\\\\"{}0]\\|u{[0-9a-fA-F]+}\\)"
     (0 (when (nth 3 (syntax-ppss)) font-lock-warning-face) prepend))
    ("{[^{}\n\"]*}"
     (0 (when (nth 3 (syntax-ppss)) font-lock-variable-name-face) prepend))))

(defvar dif-mode-syntax-table
  (let ((st (make-syntax-table)))
    (modify-syntax-entry ?_ "_" st)
    (modify-syntax-entry ?\" "\"" st)
    (modify-syntax-entry ?\\ "\\" st)
    (modify-syntax-entry ?/ ". 124b" st)
    (modify-syntax-entry ?\n "> b" st)
    st))

(defun dif-indent-line ()
  "Indent by 4 per open brace; `case'/`default' align with their `switch'."
  (interactive)
  (let ((target
         (save-excursion
           (back-to-indentation)
           (cond
            ((nth 3 (syntax-ppss)) (current-indentation))
            ((= (line-number-at-pos) 1) 0)
            ;; `}', `case' and `default' line up with the line that opened the block
            ((looking-at "}\\|case\\_>\\|default\\_>")
             (condition-case nil
                 (progn (up-list -1 t t) (current-indentation))
               (error 0)))
            (t
             (forward-line -1)
             (while (and (not (bobp)) (looking-at "[ \t]*\\(//.*\\)?$"))
               (forward-line -1))
             (back-to-indentation)
             (+ (current-indentation)
                (if (looking-at ".*\\({\\|\\_<\\(case\\|default\\)\\_>.*:\\)[ \t]*$") 4 0)))))))
    (indent-line-to target)))

;;;###autoload
(define-derived-mode dif-mode prog-mode "dif"
  "Major mode for editing difficulty source files."
  (setq-local font-lock-defaults '(dif-font-lock-keywords))
  (setq-local comment-start "// ")
  (setq-local comment-end "")
  (setq-local indent-line-function #'dif-indent-line)
  (setq-local indent-tabs-mode nil)
  (setq-local tab-width 4))

;;;###autoload
(add-to-list 'auto-mode-alist '("\\.dif\\'" . dif-mode))

(provide 'dif-mode)
;;; dif-mode.el ends here
