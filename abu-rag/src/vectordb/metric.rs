use super::Vector;
use num_traits::Zero;
use num_traits::Float;

pub trait VectorMetric : Send + Sync {
    fn score<V: Vector>(a: V, b: V) -> Result<V::Scalar, VectorMetricError>;
}

#[derive(Debug, thiserror::Error)]
pub enum VectorMetricError {
    #[error("vector len no equal {0} != {1}")]
    VectorLen(usize, usize),
}

// ============================================================================ //
//                 cosine similarity
// ============================================================================ //

pub struct Cosine;

impl Cosine {
    pub fn cosine_similarity<V: Vector>(a: V, b: V) -> Result<V::Scalar, VectorMetricError> {
        if a.len() != b.len() {
            return Err(VectorMetricError::VectorLen(a.len(), b.len()))
        }
    
        let mut dot_product = V::Scalar::zero();
        let mut norm_a = V::Scalar::zero();
        let mut norm_b = V::Scalar::zero();
    
        for (x, y) in a.into_iter().zip(b.into_iter()) {
            dot_product += x * y;
            norm_a += x * x;
            norm_b += y * y;
        }
    
        let score = if norm_a == V::Scalar::zero() || norm_b == V::Scalar::zero() {
            V::Scalar::zero()
        } else {
            dot_product / (norm_a.sqrt() * norm_b.sqrt())
        };

        Ok(score)
    }
}

impl VectorMetric for Cosine {
    #[inline]
    fn score<V: Vector>(a: V, b: V) -> Result<V::Scalar, VectorMetricError> {
        Self::cosine_similarity(a, b)
    }
}

// ============================================================================ //
//                 cosine similarity
// ============================================================================ //

pub struct L2;

impl L2 {
    pub fn l2_distance<V: Vector>(a: V, b: V) -> Result<V::Scalar, VectorMetricError> {
        if a.len() != b.len() {
            return Err(VectorMetricError::VectorLen(a.len(), b.len()))
        }

        let score = a.into_iter().zip(b.into_iter())
            .map(|(a, b)| (a - b).powi(2))
            .sum::<V::Scalar>()
            .sqrt();

        Ok(-score)
    }
}

impl VectorMetric for L2 {
    #[inline]
    fn score<V: Vector>(a: V, b: V) -> Result<V::Scalar, VectorMetricError> {
        Self::l2_distance(a, b)
    }
}

// ============================================================================ //
//                 cosine similarity
// ============================================================================ //

pub struct Dot;

impl Dot {
    pub fn dot<V: Vector>(a: V, b: V) -> Result<V::Scalar, VectorMetricError> {
        if a.len() != b.len() {
            return Err(VectorMetricError::VectorLen(a.len(), b.len()))
        }

        let score = a.into_iter().zip(b.into_iter())
            .map(|(x, y)| x * y)
            .sum();

        Ok(score)
    }
}

impl VectorMetric for Dot {
    #[inline]
    fn score<V: Vector>(a: V, b: V) -> Result<V::Scalar, VectorMetricError> {
        Self::dot(a, b)
    }
}
