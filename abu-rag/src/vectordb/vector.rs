pub trait Float :
      num_traits::Float
    + std::ops::AddAssign
    + std::ops::SubAssign
    + std::ops::MulAssign
    + std::ops::DivAssign
    + std::iter::Sum
    + Send + Sync
{
}

impl Float for f32 {}
impl Float for f64 {}

pub trait Vector: Sized {
    type Scalar: Float;

    fn scalars(self) -> impl IntoIterator<Item = Self::Scalar>;
    
    fn into_iter(self) -> impl Iterator<Item = Self::Scalar> {
        self.scalars().into_iter()
    }
    
    fn to_vec(self) -> Vec<Self::Scalar> {
        self.into_iter().collect()
    }
    
    fn len(&self) -> usize;
}

impl<F: Float> Vector for Vec<F> {
    type Scalar = F;
    
    fn scalars(self) -> impl IntoIterator<Item = Self::Scalar> {
        self
    }

    fn to_vec(self) -> Vec<Self::Scalar> {
        self
    }
    
    fn len(&self) -> usize {
        self.len()
    }
}

impl<F: Float> Vector for &Vec<F> {
    type Scalar = F;
    
    fn scalars(self) -> impl IntoIterator<Item = Self::Scalar> {
        self.iter().cloned()
    }

    fn to_vec(self) -> Vec<Self::Scalar> {
        self.clone()
    }
    
    fn len(&self) -> usize {
        Vec::len(&self)
    }
}

impl<F: Float, const N: usize> Vector for [F; N] {
    type Scalar = F;

    fn scalars(self) -> impl IntoIterator<Item = Self::Scalar> {
        self
    }

    fn len(&self) -> usize {
        N
    }
}

impl<F: Float> Vector for &[F] {
    type Scalar = F;

    fn scalars(self) -> impl IntoIterator<Item = Self::Scalar> {
        self.iter().cloned()
    }

    fn to_vec(self) -> Vec<Self::Scalar> {
        self.to_owned()
    }

    fn len(&self) -> usize {
        <[F]>::len(self)
    }
}
