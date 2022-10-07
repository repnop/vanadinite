// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use crate::{
    hash::FxHasher,
    serialize::{ReservationToken, SerializeError, Serializer},
};

use super::{sealed, Fields, Primitive};

impl sealed::Sealed for () {}
impl Fields for () {
    const ID: u64 = FxHasher::new().hash(<() as Primitive>::ID).finish();
    type Head = ();
    type Next = ();
    type Input<'a> = ();

    fn encode(_: Self::Input<'_>, _: &mut Serializer, _: &mut ReservationToken) -> Result<(), SerializeError> {
        Ok(())
    }

    fn layout() -> core::alloc::Layout {
        core::alloc::Layout::new::<()>()
    }
}

impl<A: Primitive> sealed::Sealed for (A,) {}
impl<A: Primitive> Fields for (A,) {
    const ID: u64 = FxHasher::new().hash(A::ID).hash(<Self::Next as Fields>::ID).finish();
    type Head = A;
    type Next = ();
    type Input<'a> = (A::Input<'a>,);

    fn encode(
        input: Self::Input<'_>,
        serializer: &mut Serializer,
        token: &mut ReservationToken,
    ) -> Result<(), SerializeError> {
        A::encode(input.0, serializer, token)?;

        Ok(())
    }
}

impl<A: Primitive, B: Primitive> sealed::Sealed for (A, B) {}
impl<A: Primitive, B: Primitive> Fields for (A, B) {
    const ID: u64 = FxHasher::new().hash(A::ID).hash(<Self::Next as Fields>::ID).finish();
    type Head = A;
    type Next = (B,);
    type Input<'a> = (A::Input<'a>, B::Input<'a>);

    fn encode(
        input: Self::Input<'_>,
        serializer: &mut Serializer,
        token: &mut ReservationToken,
    ) -> Result<(), SerializeError> {
        A::encode(input.0, serializer, token)?;
        B::encode(input.1, serializer, token)?;

        Ok(())
    }
}

impl<A: Primitive, B: Primitive, C: Primitive> sealed::Sealed for (A, B, C) {}
impl<A: Primitive, B: Primitive, C: Primitive> Fields for (A, B, C) {
    const ID: u64 = FxHasher::new().hash(A::ID).hash(<Self::Next as Fields>::ID).finish();
    type Head = A;
    type Next = (B, C);
    type Input<'a> = (A::Input<'a>, B::Input<'a>, C::Input<'a>);

    fn encode(
        input: Self::Input<'_>,
        serializer: &mut Serializer,
        token: &mut ReservationToken,
    ) -> Result<(), SerializeError> {
        A::encode(input.0, serializer, token)?;
        B::encode(input.1, serializer, token)?;
        C::encode(input.2, serializer, token)?;

        Ok(())
    }
}

impl<A: Primitive, B: Primitive, C: Primitive, D: Primitive> sealed::Sealed for (A, B, C, D) {}
impl<A: Primitive, B: Primitive, C: Primitive, D: Primitive> Fields for (A, B, C, D) {
    const ID: u64 = FxHasher::new().hash(A::ID).hash(<Self::Next as Fields>::ID).finish();
    type Head = A;
    type Next = (B, C, D);
    type Input<'a> = (A::Input<'a>, B::Input<'a>, C::Input<'a>, D::Input<'a>);

    fn encode(
        input: Self::Input<'_>,
        serializer: &mut Serializer,
        token: &mut ReservationToken,
    ) -> Result<(), SerializeError> {
        A::encode(input.0, serializer, token)?;
        B::encode(input.1, serializer, token)?;
        C::encode(input.2, serializer, token)?;
        D::encode(input.3, serializer, token)?;

        Ok(())
    }
}

impl<A: Primitive, B: Primitive, C: Primitive, D: Primitive, E: Primitive> sealed::Sealed for (A, B, C, D, E) {}
impl<A: Primitive, B: Primitive, C: Primitive, D: Primitive, E: Primitive> Fields for (A, B, C, D, E) {
    const ID: u64 = FxHasher::new().hash(A::ID).hash(<Self::Next as Fields>::ID).finish();
    type Head = A;
    type Next = (B, C, D, E);
    type Input<'a> = (A::Input<'a>, B::Input<'a>, C::Input<'a>, D::Input<'a>, E::Input<'a>);

    fn encode(
        input: Self::Input<'_>,
        serializer: &mut Serializer,
        token: &mut ReservationToken,
    ) -> Result<(), SerializeError> {
        A::encode(input.0, serializer, token)?;
        B::encode(input.1, serializer, token)?;
        C::encode(input.2, serializer, token)?;
        D::encode(input.3, serializer, token)?;
        E::encode(input.4, serializer, token)?;

        Ok(())
    }
}

impl<A: Primitive, B: Primitive, C: Primitive, D: Primitive, E: Primitive, F: Primitive> sealed::Sealed
    for (A, B, C, D, E, F)
{
}
impl<A: Primitive, B: Primitive, C: Primitive, D: Primitive, E: Primitive, F: Primitive> Fields for (A, B, C, D, E, F) {
    const ID: u64 = FxHasher::new().hash(A::ID).hash(<Self::Next as Fields>::ID).finish();
    type Head = A;
    type Next = (B, C, D, E, F);
    type Input<'a> = (A::Input<'a>, B::Input<'a>, C::Input<'a>, D::Input<'a>, E::Input<'a>, F::Input<'a>);

    fn encode(
        input: Self::Input<'_>,
        serializer: &mut Serializer,
        token: &mut ReservationToken,
    ) -> Result<(), SerializeError> {
        A::encode(input.0, serializer, token)?;
        B::encode(input.1, serializer, token)?;
        C::encode(input.2, serializer, token)?;
        D::encode(input.3, serializer, token)?;
        E::encode(input.4, serializer, token)?;
        F::encode(input.5, serializer, token)?;

        Ok(())
    }
}

impl<A: Primitive, B: Primitive, C: Primitive, D: Primitive, E: Primitive, F: Primitive, G: Primitive> sealed::Sealed
    for (A, B, C, D, E, F, G)
{
}
impl<A: Primitive, B: Primitive, C: Primitive, D: Primitive, E: Primitive, F: Primitive, G: Primitive> Fields
    for (A, B, C, D, E, F, G)
{
    const ID: u64 = FxHasher::new().hash(A::ID).hash(<Self::Next as Fields>::ID).finish();
    type Head = A;
    type Next = (B, C, D, E, F, G);
    type Input<'a> = (A::Input<'a>, B::Input<'a>, C::Input<'a>, D::Input<'a>, E::Input<'a>, F::Input<'a>, G::Input<'a>);

    fn encode(
        input: Self::Input<'_>,
        serializer: &mut Serializer,
        token: &mut ReservationToken,
    ) -> Result<(), SerializeError> {
        A::encode(input.0, serializer, token)?;
        B::encode(input.1, serializer, token)?;
        C::encode(input.2, serializer, token)?;
        D::encode(input.3, serializer, token)?;
        E::encode(input.4, serializer, token)?;
        F::encode(input.5, serializer, token)?;
        G::encode(input.6, serializer, token)?;

        Ok(())
    }
}

impl<
        A: Primitive,
        B: Primitive,
        C: Primitive,
        D: Primitive,
        E: Primitive,
        F: Primitive,
        G: Primitive,
        H: Primitive,
    > sealed::Sealed for (A, B, C, D, E, F, G, H)
{
}
impl<
        A: Primitive,
        B: Primitive,
        C: Primitive,
        D: Primitive,
        E: Primitive,
        F: Primitive,
        G: Primitive,
        H: Primitive,
    > Fields for (A, B, C, D, E, F, G, H)
{
    const ID: u64 = FxHasher::new().hash(A::ID).hash(<Self::Next as Fields>::ID).finish();
    type Head = A;
    type Next = (B, C, D, E, F, G, H);
    type Input<'a> = (
        A::Input<'a>,
        B::Input<'a>,
        C::Input<'a>,
        D::Input<'a>,
        E::Input<'a>,
        F::Input<'a>,
        G::Input<'a>,
        H::Input<'a>,
    );

    fn encode(
        input: Self::Input<'_>,
        serializer: &mut Serializer,
        token: &mut ReservationToken,
    ) -> Result<(), SerializeError> {
        A::encode(input.0, serializer, token)?;
        B::encode(input.1, serializer, token)?;
        C::encode(input.2, serializer, token)?;
        D::encode(input.3, serializer, token)?;
        E::encode(input.4, serializer, token)?;
        F::encode(input.5, serializer, token)?;
        G::encode(input.6, serializer, token)?;
        H::encode(input.7, serializer, token)?;

        Ok(())
    }
}

impl<
        A: Primitive,
        B: Primitive,
        C: Primitive,
        D: Primitive,
        E: Primitive,
        F: Primitive,
        G: Primitive,
        H: Primitive,
        I: Primitive,
    > sealed::Sealed for (A, B, C, D, E, F, G, H, I)
{
}
impl<
        A: Primitive,
        B: Primitive,
        C: Primitive,
        D: Primitive,
        E: Primitive,
        F: Primitive,
        G: Primitive,
        H: Primitive,
        I: Primitive,
    > Fields for (A, B, C, D, E, F, G, H, I)
{
    const ID: u64 = FxHasher::new().hash(A::ID).hash(<Self::Next as Fields>::ID).finish();
    type Head = A;
    type Next = (B, C, D, E, F, G, H, I);
    type Input<'a> = (
        A::Input<'a>,
        B::Input<'a>,
        C::Input<'a>,
        D::Input<'a>,
        E::Input<'a>,
        F::Input<'a>,
        G::Input<'a>,
        H::Input<'a>,
        I::Input<'a>,
    );

    fn encode(
        input: Self::Input<'_>,
        serializer: &mut Serializer,
        token: &mut ReservationToken,
    ) -> Result<(), SerializeError> {
        A::encode(input.0, serializer, token)?;
        B::encode(input.1, serializer, token)?;
        C::encode(input.2, serializer, token)?;
        D::encode(input.3, serializer, token)?;
        E::encode(input.4, serializer, token)?;
        F::encode(input.5, serializer, token)?;
        G::encode(input.6, serializer, token)?;
        H::encode(input.7, serializer, token)?;
        I::encode(input.8, serializer, token)?;

        Ok(())
    }
}

impl<
        A: Primitive,
        B: Primitive,
        C: Primitive,
        D: Primitive,
        E: Primitive,
        F: Primitive,
        G: Primitive,
        H: Primitive,
        I: Primitive,
        J: Primitive,
    > sealed::Sealed for (A, B, C, D, E, F, G, H, I, J)
{
}
impl<
        A: Primitive,
        B: Primitive,
        C: Primitive,
        D: Primitive,
        E: Primitive,
        F: Primitive,
        G: Primitive,
        H: Primitive,
        I: Primitive,
        J: Primitive,
    > Fields for (A, B, C, D, E, F, G, H, I, J)
{
    const ID: u64 = FxHasher::new().hash(A::ID).hash(<Self::Next as Fields>::ID).finish();
    type Head = A;
    type Next = (B, C, D, E, F, G, H, I, J);
    type Input<'a> = (
        A::Input<'a>,
        B::Input<'a>,
        C::Input<'a>,
        D::Input<'a>,
        E::Input<'a>,
        F::Input<'a>,
        G::Input<'a>,
        H::Input<'a>,
        I::Input<'a>,
        J::Input<'a>,
    );

    fn encode(
        input: Self::Input<'_>,
        serializer: &mut Serializer,
        token: &mut ReservationToken,
    ) -> Result<(), SerializeError> {
        A::encode(input.0, serializer, token)?;
        B::encode(input.1, serializer, token)?;
        C::encode(input.2, serializer, token)?;
        D::encode(input.3, serializer, token)?;
        E::encode(input.4, serializer, token)?;
        F::encode(input.5, serializer, token)?;
        G::encode(input.6, serializer, token)?;
        H::encode(input.7, serializer, token)?;
        I::encode(input.8, serializer, token)?;
        J::encode(input.9, serializer, token)?;

        Ok(())
    }
}

impl<
        A: Primitive,
        B: Primitive,
        C: Primitive,
        D: Primitive,
        E: Primitive,
        F: Primitive,
        G: Primitive,
        H: Primitive,
        I: Primitive,
        J: Primitive,
        K: Primitive,
    > sealed::Sealed for (A, B, C, D, E, F, G, H, I, J, K)
{
}
impl<
        A: Primitive,
        B: Primitive,
        C: Primitive,
        D: Primitive,
        E: Primitive,
        F: Primitive,
        G: Primitive,
        H: Primitive,
        I: Primitive,
        J: Primitive,
        K: Primitive,
    > Fields for (A, B, C, D, E, F, G, H, I, J, K)
{
    const ID: u64 = FxHasher::new().hash(A::ID).hash(<Self::Next as Fields>::ID).finish();
    type Head = A;
    type Next = (B, C, D, E, F, G, H, I, J, K);
    type Input<'a> = (
        A::Input<'a>,
        B::Input<'a>,
        C::Input<'a>,
        D::Input<'a>,
        E::Input<'a>,
        F::Input<'a>,
        G::Input<'a>,
        H::Input<'a>,
        I::Input<'a>,
        J::Input<'a>,
        K::Input<'a>,
    );

    fn encode(
        input: Self::Input<'_>,
        serializer: &mut Serializer,
        token: &mut ReservationToken,
    ) -> Result<(), SerializeError> {
        A::encode(input.0, serializer, token)?;
        B::encode(input.1, serializer, token)?;
        C::encode(input.2, serializer, token)?;
        D::encode(input.3, serializer, token)?;
        E::encode(input.4, serializer, token)?;
        F::encode(input.5, serializer, token)?;
        G::encode(input.6, serializer, token)?;
        H::encode(input.7, serializer, token)?;
        I::encode(input.8, serializer, token)?;
        J::encode(input.9, serializer, token)?;
        K::encode(input.10, serializer, token)?;

        Ok(())
    }
}

impl<
        A: Primitive,
        B: Primitive,
        C: Primitive,
        D: Primitive,
        E: Primitive,
        F: Primitive,
        G: Primitive,
        H: Primitive,
        I: Primitive,
        J: Primitive,
        K: Primitive,
        L: Primitive,
    > sealed::Sealed for (A, B, C, D, E, F, G, H, I, J, K, L)
{
}
impl<
        A: Primitive,
        B: Primitive,
        C: Primitive,
        D: Primitive,
        E: Primitive,
        F: Primitive,
        G: Primitive,
        H: Primitive,
        I: Primitive,
        J: Primitive,
        K: Primitive,
        L: Primitive,
    > Fields for (A, B, C, D, E, F, G, H, I, J, K, L)
{
    const ID: u64 = FxHasher::new().hash(A::ID).hash(<Self::Next as Fields>::ID).finish();
    type Head = A;
    type Next = (B, C, D, E, F, G, H, I, J, K, L);
    type Input<'a> = (
        A::Input<'a>,
        B::Input<'a>,
        C::Input<'a>,
        D::Input<'a>,
        E::Input<'a>,
        F::Input<'a>,
        G::Input<'a>,
        H::Input<'a>,
        I::Input<'a>,
        J::Input<'a>,
        K::Input<'a>,
        L::Input<'a>,
    );

    fn encode(
        input: Self::Input<'_>,
        serializer: &mut Serializer,
        token: &mut ReservationToken,
    ) -> Result<(), SerializeError> {
        A::encode(input.0, serializer, token)?;
        B::encode(input.1, serializer, token)?;
        C::encode(input.2, serializer, token)?;
        D::encode(input.3, serializer, token)?;
        E::encode(input.4, serializer, token)?;
        F::encode(input.5, serializer, token)?;
        G::encode(input.6, serializer, token)?;
        H::encode(input.7, serializer, token)?;
        I::encode(input.8, serializer, token)?;
        J::encode(input.9, serializer, token)?;
        K::encode(input.10, serializer, token)?;
        L::encode(input.11, serializer, token)?;

        Ok(())
    }
}

impl<
        A: Primitive,
        B: Primitive,
        C: Primitive,
        D: Primitive,
        E: Primitive,
        F: Primitive,
        G: Primitive,
        H: Primitive,
        I: Primitive,
        J: Primitive,
        K: Primitive,
        L: Primitive,
        M: Primitive,
    > sealed::Sealed for (A, B, C, D, E, F, G, H, I, J, K, L, M)
{
}
impl<
        A: Primitive,
        B: Primitive,
        C: Primitive,
        D: Primitive,
        E: Primitive,
        F: Primitive,
        G: Primitive,
        H: Primitive,
        I: Primitive,
        J: Primitive,
        K: Primitive,
        L: Primitive,
        M: Primitive,
    > Fields for (A, B, C, D, E, F, G, H, I, J, K, L, M)
{
    const ID: u64 = FxHasher::new().hash(A::ID).hash(<Self::Next as Fields>::ID).finish();
    type Head = A;
    type Next = (B, C, D, E, F, G, H, I, J, K, L, M);
    type Input<'a> = (
        A::Input<'a>,
        B::Input<'a>,
        C::Input<'a>,
        D::Input<'a>,
        E::Input<'a>,
        F::Input<'a>,
        G::Input<'a>,
        H::Input<'a>,
        I::Input<'a>,
        J::Input<'a>,
        K::Input<'a>,
        L::Input<'a>,
        M::Input<'a>,
    );

    fn encode(
        input: Self::Input<'_>,
        serializer: &mut Serializer,
        token: &mut ReservationToken,
    ) -> Result<(), SerializeError> {
        A::encode(input.0, serializer, token)?;
        B::encode(input.1, serializer, token)?;
        C::encode(input.2, serializer, token)?;
        D::encode(input.3, serializer, token)?;
        E::encode(input.4, serializer, token)?;
        F::encode(input.5, serializer, token)?;
        G::encode(input.6, serializer, token)?;
        H::encode(input.7, serializer, token)?;
        I::encode(input.8, serializer, token)?;
        J::encode(input.9, serializer, token)?;
        K::encode(input.10, serializer, token)?;
        L::encode(input.11, serializer, token)?;
        M::encode(input.12, serializer, token)?;

        Ok(())
    }
}

impl<
        A: Primitive,
        B: Primitive,
        C: Primitive,
        D: Primitive,
        E: Primitive,
        F: Primitive,
        G: Primitive,
        H: Primitive,
        I: Primitive,
        J: Primitive,
        K: Primitive,
        L: Primitive,
        M: Primitive,
        N: Primitive,
    > sealed::Sealed for (A, B, C, D, E, F, G, H, I, J, K, L, M, N)
{
}
impl<
        A: Primitive,
        B: Primitive,
        C: Primitive,
        D: Primitive,
        E: Primitive,
        F: Primitive,
        G: Primitive,
        H: Primitive,
        I: Primitive,
        J: Primitive,
        K: Primitive,
        L: Primitive,
        M: Primitive,
        N: Primitive,
    > Fields for (A, B, C, D, E, F, G, H, I, J, K, L, M, N)
{
    const ID: u64 = FxHasher::new().hash(A::ID).hash(<Self::Next as Fields>::ID).finish();
    type Head = A;
    type Next = (B, C, D, E, F, G, H, I, J, K, L, M, N);
    type Input<'a> = (
        A::Input<'a>,
        B::Input<'a>,
        C::Input<'a>,
        D::Input<'a>,
        E::Input<'a>,
        F::Input<'a>,
        G::Input<'a>,
        H::Input<'a>,
        I::Input<'a>,
        J::Input<'a>,
        K::Input<'a>,
        L::Input<'a>,
        M::Input<'a>,
        N::Input<'a>,
    );

    fn encode(
        input: Self::Input<'_>,
        serializer: &mut Serializer,
        token: &mut ReservationToken,
    ) -> Result<(), SerializeError> {
        A::encode(input.0, serializer, token)?;
        B::encode(input.1, serializer, token)?;
        C::encode(input.2, serializer, token)?;
        D::encode(input.3, serializer, token)?;
        E::encode(input.4, serializer, token)?;
        F::encode(input.5, serializer, token)?;
        G::encode(input.6, serializer, token)?;
        H::encode(input.7, serializer, token)?;
        I::encode(input.8, serializer, token)?;
        J::encode(input.9, serializer, token)?;
        K::encode(input.10, serializer, token)?;
        L::encode(input.11, serializer, token)?;
        M::encode(input.12, serializer, token)?;
        N::encode(input.13, serializer, token)?;

        Ok(())
    }
}

impl<
        A: Primitive,
        B: Primitive,
        C: Primitive,
        D: Primitive,
        E: Primitive,
        F: Primitive,
        G: Primitive,
        H: Primitive,
        I: Primitive,
        J: Primitive,
        K: Primitive,
        L: Primitive,
        M: Primitive,
        N: Primitive,
        O: Primitive,
    > sealed::Sealed for (A, B, C, D, E, F, G, H, I, J, K, L, M, N, O)
{
}
impl<
        A: Primitive,
        B: Primitive,
        C: Primitive,
        D: Primitive,
        E: Primitive,
        F: Primitive,
        G: Primitive,
        H: Primitive,
        I: Primitive,
        J: Primitive,
        K: Primitive,
        L: Primitive,
        M: Primitive,
        N: Primitive,
        O: Primitive,
    > Fields for (A, B, C, D, E, F, G, H, I, J, K, L, M, N, O)
{
    const ID: u64 = FxHasher::new().hash(A::ID).hash(<Self::Next as Fields>::ID).finish();
    type Head = A;
    type Next = (B, C, D, E, F, G, H, I, J, K, L, M, N, O);
    type Input<'a> = (
        A::Input<'a>,
        B::Input<'a>,
        C::Input<'a>,
        D::Input<'a>,
        E::Input<'a>,
        F::Input<'a>,
        G::Input<'a>,
        H::Input<'a>,
        I::Input<'a>,
        J::Input<'a>,
        K::Input<'a>,
        L::Input<'a>,
        M::Input<'a>,
        N::Input<'a>,
        O::Input<'a>,
    );

    fn encode(
        input: Self::Input<'_>,
        serializer: &mut Serializer,
        token: &mut ReservationToken,
    ) -> Result<(), SerializeError> {
        A::encode(input.0, serializer, token)?;
        B::encode(input.1, serializer, token)?;
        C::encode(input.2, serializer, token)?;
        D::encode(input.3, serializer, token)?;
        E::encode(input.4, serializer, token)?;
        F::encode(input.5, serializer, token)?;
        G::encode(input.6, serializer, token)?;
        H::encode(input.7, serializer, token)?;
        I::encode(input.8, serializer, token)?;
        J::encode(input.9, serializer, token)?;
        K::encode(input.10, serializer, token)?;
        L::encode(input.11, serializer, token)?;
        M::encode(input.12, serializer, token)?;
        N::encode(input.13, serializer, token)?;
        O::encode(input.14, serializer, token)?;

        Ok(())
    }
}

impl<
        A: Primitive,
        B: Primitive,
        C: Primitive,
        D: Primitive,
        E: Primitive,
        F: Primitive,
        G: Primitive,
        H: Primitive,
        I: Primitive,
        J: Primitive,
        K: Primitive,
        L: Primitive,
        M: Primitive,
        N: Primitive,
        O: Primitive,
        P: Primitive,
    > sealed::Sealed for (A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P)
{
}
impl<
        A: Primitive,
        B: Primitive,
        C: Primitive,
        D: Primitive,
        E: Primitive,
        F: Primitive,
        G: Primitive,
        H: Primitive,
        I: Primitive,
        J: Primitive,
        K: Primitive,
        L: Primitive,
        M: Primitive,
        N: Primitive,
        O: Primitive,
        P: Primitive,
    > Fields for (A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P)
{
    const ID: u64 = FxHasher::new().hash(A::ID).hash(<Self::Next as Fields>::ID).finish();
    type Head = A;
    type Next = (B, C, D, E, F, G, H, I, J, K, L, M, N, O, P);
    type Input<'a> = (
        A::Input<'a>,
        B::Input<'a>,
        C::Input<'a>,
        D::Input<'a>,
        E::Input<'a>,
        F::Input<'a>,
        G::Input<'a>,
        H::Input<'a>,
        I::Input<'a>,
        J::Input<'a>,
        K::Input<'a>,
        L::Input<'a>,
        M::Input<'a>,
        N::Input<'a>,
        O::Input<'a>,
        P::Input<'a>,
    );

    fn encode(
        input: Self::Input<'_>,
        serializer: &mut Serializer,
        token: &mut ReservationToken,
    ) -> Result<(), SerializeError> {
        A::encode(input.0, serializer, token)?;
        B::encode(input.1, serializer, token)?;
        C::encode(input.2, serializer, token)?;
        D::encode(input.3, serializer, token)?;
        E::encode(input.4, serializer, token)?;
        F::encode(input.5, serializer, token)?;
        G::encode(input.6, serializer, token)?;
        H::encode(input.7, serializer, token)?;
        I::encode(input.8, serializer, token)?;
        J::encode(input.9, serializer, token)?;
        K::encode(input.10, serializer, token)?;
        L::encode(input.11, serializer, token)?;
        M::encode(input.12, serializer, token)?;
        N::encode(input.13, serializer, token)?;
        O::encode(input.14, serializer, token)?;
        P::encode(input.15, serializer, token)?;

        Ok(())
    }
}

impl<
        A: Primitive,
        B: Primitive,
        C: Primitive,
        D: Primitive,
        E: Primitive,
        F: Primitive,
        G: Primitive,
        H: Primitive,
        I: Primitive,
        J: Primitive,
        K: Primitive,
        L: Primitive,
        M: Primitive,
        N: Primitive,
        O: Primitive,
        P: Primitive,
        Q: Primitive,
    > sealed::Sealed for (A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q)
{
}
impl<
        A: Primitive,
        B: Primitive,
        C: Primitive,
        D: Primitive,
        E: Primitive,
        F: Primitive,
        G: Primitive,
        H: Primitive,
        I: Primitive,
        J: Primitive,
        K: Primitive,
        L: Primitive,
        M: Primitive,
        N: Primitive,
        O: Primitive,
        P: Primitive,
        Q: Primitive,
    > Fields for (A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q)
{
    const ID: u64 = FxHasher::new().hash(A::ID).hash(<Self::Next as Fields>::ID).finish();
    type Head = A;
    type Next = (B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q);
    type Input<'a> = (
        A::Input<'a>,
        B::Input<'a>,
        C::Input<'a>,
        D::Input<'a>,
        E::Input<'a>,
        F::Input<'a>,
        G::Input<'a>,
        H::Input<'a>,
        I::Input<'a>,
        J::Input<'a>,
        K::Input<'a>,
        L::Input<'a>,
        M::Input<'a>,
        N::Input<'a>,
        O::Input<'a>,
        P::Input<'a>,
        Q::Input<'a>,
    );

    fn encode(
        input: Self::Input<'_>,
        serializer: &mut Serializer,
        token: &mut ReservationToken,
    ) -> Result<(), SerializeError> {
        A::encode(input.0, serializer, token)?;
        B::encode(input.1, serializer, token)?;
        C::encode(input.2, serializer, token)?;
        D::encode(input.3, serializer, token)?;
        E::encode(input.4, serializer, token)?;
        F::encode(input.5, serializer, token)?;
        G::encode(input.6, serializer, token)?;
        H::encode(input.7, serializer, token)?;
        I::encode(input.8, serializer, token)?;
        J::encode(input.9, serializer, token)?;
        K::encode(input.10, serializer, token)?;
        L::encode(input.11, serializer, token)?;
        M::encode(input.12, serializer, token)?;
        N::encode(input.13, serializer, token)?;
        O::encode(input.14, serializer, token)?;
        P::encode(input.15, serializer, token)?;
        Q::encode(input.16, serializer, token)?;

        Ok(())
    }
}

impl<
        A: Primitive,
        B: Primitive,
        C: Primitive,
        D: Primitive,
        E: Primitive,
        F: Primitive,
        G: Primitive,
        H: Primitive,
        I: Primitive,
        J: Primitive,
        K: Primitive,
        L: Primitive,
        M: Primitive,
        N: Primitive,
        O: Primitive,
        P: Primitive,
        Q: Primitive,
        R: Primitive,
    > sealed::Sealed for (A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R)
{
}
impl<
        A: Primitive,
        B: Primitive,
        C: Primitive,
        D: Primitive,
        E: Primitive,
        F: Primitive,
        G: Primitive,
        H: Primitive,
        I: Primitive,
        J: Primitive,
        K: Primitive,
        L: Primitive,
        M: Primitive,
        N: Primitive,
        O: Primitive,
        P: Primitive,
        Q: Primitive,
        R: Primitive,
    > Fields for (A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R)
{
    const ID: u64 = FxHasher::new().hash(A::ID).hash(<Self::Next as Fields>::ID).finish();
    type Head = A;
    type Next = (B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R);
    type Input<'a> = (
        A::Input<'a>,
        B::Input<'a>,
        C::Input<'a>,
        D::Input<'a>,
        E::Input<'a>,
        F::Input<'a>,
        G::Input<'a>,
        H::Input<'a>,
        I::Input<'a>,
        J::Input<'a>,
        K::Input<'a>,
        L::Input<'a>,
        M::Input<'a>,
        N::Input<'a>,
        O::Input<'a>,
        P::Input<'a>,
        Q::Input<'a>,
        R::Input<'a>,
    );

    fn encode(
        input: Self::Input<'_>,
        serializer: &mut Serializer,
        token: &mut ReservationToken,
    ) -> Result<(), SerializeError> {
        A::encode(input.0, serializer, token)?;
        B::encode(input.1, serializer, token)?;
        C::encode(input.2, serializer, token)?;
        D::encode(input.3, serializer, token)?;
        E::encode(input.4, serializer, token)?;
        F::encode(input.5, serializer, token)?;
        G::encode(input.6, serializer, token)?;
        H::encode(input.7, serializer, token)?;
        I::encode(input.8, serializer, token)?;
        J::encode(input.9, serializer, token)?;
        K::encode(input.10, serializer, token)?;
        L::encode(input.11, serializer, token)?;
        M::encode(input.12, serializer, token)?;
        N::encode(input.13, serializer, token)?;
        O::encode(input.14, serializer, token)?;
        P::encode(input.15, serializer, token)?;
        Q::encode(input.16, serializer, token)?;
        R::encode(input.17, serializer, token)?;

        Ok(())
    }
}

impl<
        A: Primitive,
        B: Primitive,
        C: Primitive,
        D: Primitive,
        E: Primitive,
        F: Primitive,
        G: Primitive,
        H: Primitive,
        I: Primitive,
        J: Primitive,
        K: Primitive,
        L: Primitive,
        M: Primitive,
        N: Primitive,
        O: Primitive,
        P: Primitive,
        Q: Primitive,
        R: Primitive,
        S: Primitive,
    > sealed::Sealed for (A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S)
{
}
impl<
        A: Primitive,
        B: Primitive,
        C: Primitive,
        D: Primitive,
        E: Primitive,
        F: Primitive,
        G: Primitive,
        H: Primitive,
        I: Primitive,
        J: Primitive,
        K: Primitive,
        L: Primitive,
        M: Primitive,
        N: Primitive,
        O: Primitive,
        P: Primitive,
        Q: Primitive,
        R: Primitive,
        S: Primitive,
    > Fields for (A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S)
{
    const ID: u64 = FxHasher::new().hash(A::ID).hash(<Self::Next as Fields>::ID).finish();
    type Head = A;
    type Next = (B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S);
    type Input<'a> = (
        A::Input<'a>,
        B::Input<'a>,
        C::Input<'a>,
        D::Input<'a>,
        E::Input<'a>,
        F::Input<'a>,
        G::Input<'a>,
        H::Input<'a>,
        I::Input<'a>,
        J::Input<'a>,
        K::Input<'a>,
        L::Input<'a>,
        M::Input<'a>,
        N::Input<'a>,
        O::Input<'a>,
        P::Input<'a>,
        Q::Input<'a>,
        R::Input<'a>,
        S::Input<'a>,
    );

    fn encode(
        input: Self::Input<'_>,
        serializer: &mut Serializer,
        token: &mut ReservationToken,
    ) -> Result<(), SerializeError> {
        A::encode(input.0, serializer, token)?;
        B::encode(input.1, serializer, token)?;
        C::encode(input.2, serializer, token)?;
        D::encode(input.3, serializer, token)?;
        E::encode(input.4, serializer, token)?;
        F::encode(input.5, serializer, token)?;
        G::encode(input.6, serializer, token)?;
        H::encode(input.7, serializer, token)?;
        I::encode(input.8, serializer, token)?;
        J::encode(input.9, serializer, token)?;
        K::encode(input.10, serializer, token)?;
        L::encode(input.11, serializer, token)?;
        M::encode(input.12, serializer, token)?;
        N::encode(input.13, serializer, token)?;
        O::encode(input.14, serializer, token)?;
        P::encode(input.15, serializer, token)?;
        Q::encode(input.16, serializer, token)?;
        R::encode(input.17, serializer, token)?;
        S::encode(input.18, serializer, token)?;

        Ok(())
    }
}

impl<
        A: Primitive,
        B: Primitive,
        C: Primitive,
        D: Primitive,
        E: Primitive,
        F: Primitive,
        G: Primitive,
        H: Primitive,
        I: Primitive,
        J: Primitive,
        K: Primitive,
        L: Primitive,
        M: Primitive,
        N: Primitive,
        O: Primitive,
        P: Primitive,
        Q: Primitive,
        R: Primitive,
        S: Primitive,
        T: Primitive,
    > sealed::Sealed for (A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T)
{
}
impl<
        A: Primitive,
        B: Primitive,
        C: Primitive,
        D: Primitive,
        E: Primitive,
        F: Primitive,
        G: Primitive,
        H: Primitive,
        I: Primitive,
        J: Primitive,
        K: Primitive,
        L: Primitive,
        M: Primitive,
        N: Primitive,
        O: Primitive,
        P: Primitive,
        Q: Primitive,
        R: Primitive,
        S: Primitive,
        T: Primitive,
    > Fields for (A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T)
{
    const ID: u64 = FxHasher::new().hash(A::ID).hash(<Self::Next as Fields>::ID).finish();
    type Head = A;
    type Next = (B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T);
    type Input<'a> = (
        A::Input<'a>,
        B::Input<'a>,
        C::Input<'a>,
        D::Input<'a>,
        E::Input<'a>,
        F::Input<'a>,
        G::Input<'a>,
        H::Input<'a>,
        I::Input<'a>,
        J::Input<'a>,
        K::Input<'a>,
        L::Input<'a>,
        M::Input<'a>,
        N::Input<'a>,
        O::Input<'a>,
        P::Input<'a>,
        Q::Input<'a>,
        R::Input<'a>,
        S::Input<'a>,
        T::Input<'a>,
    );

    fn encode(
        input: Self::Input<'_>,
        serializer: &mut Serializer,
        token: &mut ReservationToken,
    ) -> Result<(), SerializeError> {
        A::encode(input.0, serializer, token)?;
        B::encode(input.1, serializer, token)?;
        C::encode(input.2, serializer, token)?;
        D::encode(input.3, serializer, token)?;
        E::encode(input.4, serializer, token)?;
        F::encode(input.5, serializer, token)?;
        G::encode(input.6, serializer, token)?;
        H::encode(input.7, serializer, token)?;
        I::encode(input.8, serializer, token)?;
        J::encode(input.9, serializer, token)?;
        K::encode(input.10, serializer, token)?;
        L::encode(input.11, serializer, token)?;
        M::encode(input.12, serializer, token)?;
        N::encode(input.13, serializer, token)?;
        O::encode(input.14, serializer, token)?;
        P::encode(input.15, serializer, token)?;
        Q::encode(input.16, serializer, token)?;
        R::encode(input.17, serializer, token)?;
        S::encode(input.18, serializer, token)?;
        T::encode(input.19, serializer, token)?;

        Ok(())
    }
}

impl<
        A: Primitive,
        B: Primitive,
        C: Primitive,
        D: Primitive,
        E: Primitive,
        F: Primitive,
        G: Primitive,
        H: Primitive,
        I: Primitive,
        J: Primitive,
        K: Primitive,
        L: Primitive,
        M: Primitive,
        N: Primitive,
        O: Primitive,
        P: Primitive,
        Q: Primitive,
        R: Primitive,
        S: Primitive,
        T: Primitive,
        U: Primitive,
    > sealed::Sealed for (A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T, U)
{
}
impl<
        A: Primitive,
        B: Primitive,
        C: Primitive,
        D: Primitive,
        E: Primitive,
        F: Primitive,
        G: Primitive,
        H: Primitive,
        I: Primitive,
        J: Primitive,
        K: Primitive,
        L: Primitive,
        M: Primitive,
        N: Primitive,
        O: Primitive,
        P: Primitive,
        Q: Primitive,
        R: Primitive,
        S: Primitive,
        T: Primitive,
        U: Primitive,
    > Fields for (A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T, U)
{
    const ID: u64 = FxHasher::new().hash(A::ID).hash(<Self::Next as Fields>::ID).finish();
    type Head = A;
    type Next = (B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T, U);
    type Input<'a> = (
        A::Input<'a>,
        B::Input<'a>,
        C::Input<'a>,
        D::Input<'a>,
        E::Input<'a>,
        F::Input<'a>,
        G::Input<'a>,
        H::Input<'a>,
        I::Input<'a>,
        J::Input<'a>,
        K::Input<'a>,
        L::Input<'a>,
        M::Input<'a>,
        N::Input<'a>,
        O::Input<'a>,
        P::Input<'a>,
        Q::Input<'a>,
        R::Input<'a>,
        S::Input<'a>,
        T::Input<'a>,
        U::Input<'a>,
    );

    fn encode(
        input: Self::Input<'_>,
        serializer: &mut Serializer,
        token: &mut ReservationToken,
    ) -> Result<(), SerializeError> {
        A::encode(input.0, serializer, token)?;
        B::encode(input.1, serializer, token)?;
        C::encode(input.2, serializer, token)?;
        D::encode(input.3, serializer, token)?;
        E::encode(input.4, serializer, token)?;
        F::encode(input.5, serializer, token)?;
        G::encode(input.6, serializer, token)?;
        H::encode(input.7, serializer, token)?;
        I::encode(input.8, serializer, token)?;
        J::encode(input.9, serializer, token)?;
        K::encode(input.10, serializer, token)?;
        L::encode(input.11, serializer, token)?;
        M::encode(input.12, serializer, token)?;
        N::encode(input.13, serializer, token)?;
        O::encode(input.14, serializer, token)?;
        P::encode(input.15, serializer, token)?;
        Q::encode(input.16, serializer, token)?;
        R::encode(input.17, serializer, token)?;
        S::encode(input.18, serializer, token)?;
        T::encode(input.19, serializer, token)?;
        U::encode(input.20, serializer, token)?;

        Ok(())
    }
}

impl<
        A: Primitive,
        B: Primitive,
        C: Primitive,
        D: Primitive,
        E: Primitive,
        F: Primitive,
        G: Primitive,
        H: Primitive,
        I: Primitive,
        J: Primitive,
        K: Primitive,
        L: Primitive,
        M: Primitive,
        N: Primitive,
        O: Primitive,
        P: Primitive,
        Q: Primitive,
        R: Primitive,
        S: Primitive,
        T: Primitive,
        U: Primitive,
        V: Primitive,
    > sealed::Sealed for (A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T, U, V)
{
}
impl<
        A: Primitive,
        B: Primitive,
        C: Primitive,
        D: Primitive,
        E: Primitive,
        F: Primitive,
        G: Primitive,
        H: Primitive,
        I: Primitive,
        J: Primitive,
        K: Primitive,
        L: Primitive,
        M: Primitive,
        N: Primitive,
        O: Primitive,
        P: Primitive,
        Q: Primitive,
        R: Primitive,
        S: Primitive,
        T: Primitive,
        U: Primitive,
        V: Primitive,
    > Fields for (A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T, U, V)
{
    const ID: u64 = FxHasher::new().hash(A::ID).hash(<Self::Next as Fields>::ID).finish();
    type Head = A;
    type Next = (B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T, U, V);
    type Input<'a> = (
        A::Input<'a>,
        B::Input<'a>,
        C::Input<'a>,
        D::Input<'a>,
        E::Input<'a>,
        F::Input<'a>,
        G::Input<'a>,
        H::Input<'a>,
        I::Input<'a>,
        J::Input<'a>,
        K::Input<'a>,
        L::Input<'a>,
        M::Input<'a>,
        N::Input<'a>,
        O::Input<'a>,
        P::Input<'a>,
        Q::Input<'a>,
        R::Input<'a>,
        S::Input<'a>,
        T::Input<'a>,
        U::Input<'a>,
        V::Input<'a>,
    );

    fn encode(
        input: Self::Input<'_>,
        serializer: &mut Serializer,
        token: &mut ReservationToken,
    ) -> Result<(), SerializeError> {
        A::encode(input.0, serializer, token)?;
        B::encode(input.1, serializer, token)?;
        C::encode(input.2, serializer, token)?;
        D::encode(input.3, serializer, token)?;
        E::encode(input.4, serializer, token)?;
        F::encode(input.5, serializer, token)?;
        G::encode(input.6, serializer, token)?;
        H::encode(input.7, serializer, token)?;
        I::encode(input.8, serializer, token)?;
        J::encode(input.9, serializer, token)?;
        K::encode(input.10, serializer, token)?;
        L::encode(input.11, serializer, token)?;
        M::encode(input.12, serializer, token)?;
        N::encode(input.13, serializer, token)?;
        O::encode(input.14, serializer, token)?;
        P::encode(input.15, serializer, token)?;
        Q::encode(input.16, serializer, token)?;
        R::encode(input.17, serializer, token)?;
        S::encode(input.18, serializer, token)?;
        T::encode(input.19, serializer, token)?;
        U::encode(input.20, serializer, token)?;
        V::encode(input.21, serializer, token)?;

        Ok(())
    }
}

impl<
        A: Primitive,
        B: Primitive,
        C: Primitive,
        D: Primitive,
        E: Primitive,
        F: Primitive,
        G: Primitive,
        H: Primitive,
        I: Primitive,
        J: Primitive,
        K: Primitive,
        L: Primitive,
        M: Primitive,
        N: Primitive,
        O: Primitive,
        P: Primitive,
        Q: Primitive,
        R: Primitive,
        S: Primitive,
        T: Primitive,
        U: Primitive,
        V: Primitive,
        W: Primitive,
    > sealed::Sealed for (A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T, U, V, W)
{
}
impl<
        A: Primitive,
        B: Primitive,
        C: Primitive,
        D: Primitive,
        E: Primitive,
        F: Primitive,
        G: Primitive,
        H: Primitive,
        I: Primitive,
        J: Primitive,
        K: Primitive,
        L: Primitive,
        M: Primitive,
        N: Primitive,
        O: Primitive,
        P: Primitive,
        Q: Primitive,
        R: Primitive,
        S: Primitive,
        T: Primitive,
        U: Primitive,
        V: Primitive,
        W: Primitive,
    > Fields for (A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T, U, V, W)
{
    const ID: u64 = FxHasher::new().hash(A::ID).hash(<Self::Next as Fields>::ID).finish();
    type Head = A;
    type Next = (B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T, U, V, W);
    type Input<'a> = (
        A::Input<'a>,
        B::Input<'a>,
        C::Input<'a>,
        D::Input<'a>,
        E::Input<'a>,
        F::Input<'a>,
        G::Input<'a>,
        H::Input<'a>,
        I::Input<'a>,
        J::Input<'a>,
        K::Input<'a>,
        L::Input<'a>,
        M::Input<'a>,
        N::Input<'a>,
        O::Input<'a>,
        P::Input<'a>,
        Q::Input<'a>,
        R::Input<'a>,
        S::Input<'a>,
        T::Input<'a>,
        U::Input<'a>,
        V::Input<'a>,
        W::Input<'a>,
    );

    fn encode(
        input: Self::Input<'_>,
        serializer: &mut Serializer,
        token: &mut ReservationToken,
    ) -> Result<(), SerializeError> {
        A::encode(input.0, serializer, token)?;
        B::encode(input.1, serializer, token)?;
        C::encode(input.2, serializer, token)?;
        D::encode(input.3, serializer, token)?;
        E::encode(input.4, serializer, token)?;
        F::encode(input.5, serializer, token)?;
        G::encode(input.6, serializer, token)?;
        H::encode(input.7, serializer, token)?;
        I::encode(input.8, serializer, token)?;
        J::encode(input.9, serializer, token)?;
        K::encode(input.10, serializer, token)?;
        L::encode(input.11, serializer, token)?;
        M::encode(input.12, serializer, token)?;
        N::encode(input.13, serializer, token)?;
        O::encode(input.14, serializer, token)?;
        P::encode(input.15, serializer, token)?;
        Q::encode(input.16, serializer, token)?;
        R::encode(input.17, serializer, token)?;
        S::encode(input.18, serializer, token)?;
        T::encode(input.19, serializer, token)?;
        U::encode(input.20, serializer, token)?;
        V::encode(input.21, serializer, token)?;
        W::encode(input.22, serializer, token)?;

        Ok(())
    }
}

impl<
        A: Primitive,
        B: Primitive,
        C: Primitive,
        D: Primitive,
        E: Primitive,
        F: Primitive,
        G: Primitive,
        H: Primitive,
        I: Primitive,
        J: Primitive,
        K: Primitive,
        L: Primitive,
        M: Primitive,
        N: Primitive,
        O: Primitive,
        P: Primitive,
        Q: Primitive,
        R: Primitive,
        S: Primitive,
        T: Primitive,
        U: Primitive,
        V: Primitive,
        W: Primitive,
        X: Primitive,
    > sealed::Sealed for (A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T, U, V, W, X)
{
}
impl<
        A: Primitive,
        B: Primitive,
        C: Primitive,
        D: Primitive,
        E: Primitive,
        F: Primitive,
        G: Primitive,
        H: Primitive,
        I: Primitive,
        J: Primitive,
        K: Primitive,
        L: Primitive,
        M: Primitive,
        N: Primitive,
        O: Primitive,
        P: Primitive,
        Q: Primitive,
        R: Primitive,
        S: Primitive,
        T: Primitive,
        U: Primitive,
        V: Primitive,
        W: Primitive,
        X: Primitive,
    > Fields for (A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T, U, V, W, X)
{
    const ID: u64 = FxHasher::new().hash(A::ID).hash(<Self::Next as Fields>::ID).finish();
    type Head = A;
    type Next = (B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T, U, V, W, X);
    type Input<'a> = (
        A::Input<'a>,
        B::Input<'a>,
        C::Input<'a>,
        D::Input<'a>,
        E::Input<'a>,
        F::Input<'a>,
        G::Input<'a>,
        H::Input<'a>,
        I::Input<'a>,
        J::Input<'a>,
        K::Input<'a>,
        L::Input<'a>,
        M::Input<'a>,
        N::Input<'a>,
        O::Input<'a>,
        P::Input<'a>,
        Q::Input<'a>,
        R::Input<'a>,
        S::Input<'a>,
        T::Input<'a>,
        U::Input<'a>,
        V::Input<'a>,
        W::Input<'a>,
        X::Input<'a>,
    );

    fn encode(
        input: Self::Input<'_>,
        serializer: &mut Serializer,
        token: &mut ReservationToken,
    ) -> Result<(), SerializeError> {
        A::encode(input.0, serializer, token)?;
        B::encode(input.1, serializer, token)?;
        C::encode(input.2, serializer, token)?;
        D::encode(input.3, serializer, token)?;
        E::encode(input.4, serializer, token)?;
        F::encode(input.5, serializer, token)?;
        G::encode(input.6, serializer, token)?;
        H::encode(input.7, serializer, token)?;
        I::encode(input.8, serializer, token)?;
        J::encode(input.9, serializer, token)?;
        K::encode(input.10, serializer, token)?;
        L::encode(input.11, serializer, token)?;
        M::encode(input.12, serializer, token)?;
        N::encode(input.13, serializer, token)?;
        O::encode(input.14, serializer, token)?;
        P::encode(input.15, serializer, token)?;
        Q::encode(input.16, serializer, token)?;
        R::encode(input.17, serializer, token)?;
        S::encode(input.18, serializer, token)?;
        T::encode(input.19, serializer, token)?;
        U::encode(input.20, serializer, token)?;
        V::encode(input.21, serializer, token)?;
        W::encode(input.22, serializer, token)?;
        X::encode(input.23, serializer, token)?;

        Ok(())
    }
}

impl<
        A: Primitive,
        B: Primitive,
        C: Primitive,
        D: Primitive,
        E: Primitive,
        F: Primitive,
        G: Primitive,
        H: Primitive,
        I: Primitive,
        J: Primitive,
        K: Primitive,
        L: Primitive,
        M: Primitive,
        N: Primitive,
        O: Primitive,
        P: Primitive,
        Q: Primitive,
        R: Primitive,
        S: Primitive,
        T: Primitive,
        U: Primitive,
        V: Primitive,
        W: Primitive,
        X: Primitive,
        Y: Primitive,
    > sealed::Sealed for (A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T, U, V, W, X, Y)
{
}
impl<
        A: Primitive,
        B: Primitive,
        C: Primitive,
        D: Primitive,
        E: Primitive,
        F: Primitive,
        G: Primitive,
        H: Primitive,
        I: Primitive,
        J: Primitive,
        K: Primitive,
        L: Primitive,
        M: Primitive,
        N: Primitive,
        O: Primitive,
        P: Primitive,
        Q: Primitive,
        R: Primitive,
        S: Primitive,
        T: Primitive,
        U: Primitive,
        V: Primitive,
        W: Primitive,
        X: Primitive,
        Y: Primitive,
    > Fields for (A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T, U, V, W, X, Y)
{
    const ID: u64 = FxHasher::new().hash(A::ID).hash(<Self::Next as Fields>::ID).finish();
    type Head = A;
    type Next = (B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T, U, V, W, X, Y);
    type Input<'a> = (
        A::Input<'a>,
        B::Input<'a>,
        C::Input<'a>,
        D::Input<'a>,
        E::Input<'a>,
        F::Input<'a>,
        G::Input<'a>,
        H::Input<'a>,
        I::Input<'a>,
        J::Input<'a>,
        K::Input<'a>,
        L::Input<'a>,
        M::Input<'a>,
        N::Input<'a>,
        O::Input<'a>,
        P::Input<'a>,
        Q::Input<'a>,
        R::Input<'a>,
        S::Input<'a>,
        T::Input<'a>,
        U::Input<'a>,
        V::Input<'a>,
        W::Input<'a>,
        X::Input<'a>,
        Y::Input<'a>,
    );

    fn encode(
        input: Self::Input<'_>,
        serializer: &mut Serializer,
        token: &mut ReservationToken,
    ) -> Result<(), SerializeError> {
        A::encode(input.0, serializer, token)?;
        B::encode(input.1, serializer, token)?;
        C::encode(input.2, serializer, token)?;
        D::encode(input.3, serializer, token)?;
        E::encode(input.4, serializer, token)?;
        F::encode(input.5, serializer, token)?;
        G::encode(input.6, serializer, token)?;
        H::encode(input.7, serializer, token)?;
        I::encode(input.8, serializer, token)?;
        J::encode(input.9, serializer, token)?;
        K::encode(input.10, serializer, token)?;
        L::encode(input.11, serializer, token)?;
        M::encode(input.12, serializer, token)?;
        N::encode(input.13, serializer, token)?;
        O::encode(input.14, serializer, token)?;
        P::encode(input.15, serializer, token)?;
        Q::encode(input.16, serializer, token)?;
        R::encode(input.17, serializer, token)?;
        S::encode(input.18, serializer, token)?;
        T::encode(input.19, serializer, token)?;
        U::encode(input.20, serializer, token)?;
        V::encode(input.21, serializer, token)?;
        W::encode(input.22, serializer, token)?;
        X::encode(input.23, serializer, token)?;
        Y::encode(input.24, serializer, token)?;

        Ok(())
    }
}
