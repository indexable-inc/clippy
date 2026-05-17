use clippy_utils::diagnostics::span_lint_and_help;
use clippy_utils::res::MaybeDef;
use clippy_utils::sym;
use rustc_hir::{Expr, ExprKind};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_middle::ty;
use rustc_session::declare_lint_pass;

declare_clippy_lint! {
    /// ### What it does
    /// Catches `.unwrap_or(0)`, `.unwrap_or(T::MAX)`, `.unwrap_or(T::MIN)`,
    /// and `.unwrap_or_default()` on `Result<_, TryFromIntError>` — i.e. the
    /// result of fallible integer conversions via `TryFrom`/`TryInto`.
    ///
    /// ### Why restrict this?
    /// Silently clamping or zeroing an out-of-range integer conversion is
    /// almost never the intended behavior when strict correctness is
    /// required. If the value does not fit, it is almost always a hard
    /// error that should be propagated with `?`.
    ///
    /// ### Example
    /// ```rust,ignore
    /// let n: u8 = u8::try_from(big_value).unwrap_or(0);
    /// let n: i16 = i16::try_from(x).unwrap_or(i16::MAX);
    /// let n: u32 = u32::try_from(x).unwrap_or_default();
    /// ```
    /// Use instead:
    /// ```rust,ignore
    /// let n: u8 = u8::try_from(big_value)?;
    /// ```
    #[clippy::version = "1.86.0"]
    pub FALLIBLE_INT_FALLBACK,
    restriction,
    "`.unwrap_or` / `.unwrap_or_default` on fallible integer conversion silently loses data"
}

declare_lint_pass!(FallibleIntFallback => [FALLIBLE_INT_FALLBACK]);

/// Returns `true` if `ty` is `std::num::TryFromIntError`.
fn is_try_from_int_error(cx: &LateContext<'_>, ty: ty::Ty<'_>) -> bool {
    if let ty::Adt(adt_def, _) = ty.kind() {
        let path = cx.tcx.def_path_str(adt_def.did());
        path == "core::num::error::TryFromIntError"
            || path == "std::num::TryFromIntError"
    } else {
        false
    }
}

/// If `recv_ty` is `Result<T, E>`, returns `E`.
fn result_error_ty<'tcx>(
    cx: &LateContext<'tcx>,
    recv_ty: ty::Ty<'tcx>,
) -> Option<ty::Ty<'tcx>> {
    if recv_ty.is_diag_item(cx, sym::Result) {
        if let ty::Adt(_, args) = recv_ty.kind() {
            return Some(args.type_at(1));
        }
    }
    None
}

impl LateLintPass<'_> for FallibleIntFallback {
    fn check_expr(&mut self, cx: &LateContext<'_>, expr: &Expr<'_>) {
        if expr.span.in_external_macro(cx.sess().source_map()) {
            return;
        }

        let ExprKind::MethodCall(method, recv, _, _) = expr.kind else {
            return;
        };

        let is_unwrap_or = method.ident.name == sym::unwrap_or;
        let is_unwrap_or_default = method.ident.name == sym::unwrap_or_default;

        if !is_unwrap_or && !is_unwrap_or_default {
            return;
        }

        let recv_ty = cx.typeck_results().expr_ty(recv);
        let Some(err_ty) = result_error_ty(cx, recv_ty) else {
            return;
        };

        if !is_try_from_int_error(cx, err_ty) {
            return;
        }

        let method_name = if is_unwrap_or_default {
            "unwrap_or_default()"
        } else {
            "unwrap_or(...)"
        };

        span_lint_and_help(
            cx,
            FALLIBLE_INT_FALLBACK,
            expr.span,
            format!(
                "`.{method_name}` on a fallible integer conversion silently \
                 discards an out-of-range error"
            ),
            None,
            "propagate the error with `?` instead; if the value does not fit, \
             it is almost certainly a hard error",
        );
    }
}
