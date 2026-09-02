// OC04-E07 compile-fail snippet source.
//
// This file is deliberately nested so Cargo does not auto-discover it as an
// integration test. `verified_prior_compile_gate` compiles it directly with
// rustc and requires E0616: the VerifiedPrior artifact field is private.
extern crate contextmesh_salience;

use contextmesh_salience::oc04_selection::VerifiedPrior;

fn privacy_violation(token: &VerifiedPrior) {
    let _artifact = &token.prior;
}
