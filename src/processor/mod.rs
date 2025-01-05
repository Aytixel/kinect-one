pub mod depth;
pub mod rgb;

use std::{future::Future, marker::PhantomData};

use crate::Error;

pub trait ProcessTrait: Sized {
    fn process<O, P: ProcessorTrait<Self, O>>(
        self,
        processor: &P,
    ) -> impl Future<Output = Result<O, Error>> {
        async {
            processor
                .process(self)
                .await
                .map_err(|error| Error::Processing(error))
        }
    }
}

pub trait ProcessorTrait<I, O> {
    fn process(&self, input: I) -> impl Future<Output = Result<O, Box<dyn std::error::Error>>>;

    fn pipe<'a, 'b, T, P>(&'a self, processor: &'b P) -> PipedProcessor<'a, 'b, I, O, T, Self, P>
    where
        Self: Sized,
        P: ProcessorTrait<O, T>,
    {
        PipedProcessor {
            _input: PhantomData::default(),
            _tmp: PhantomData::default(),
            _output: PhantomData::default(),
            processor1: self,
            processor2: processor,
        }
    }
}

pub struct NoopProcessor;

impl<T> ProcessorTrait<T, ()> for NoopProcessor {
    async fn process(&self, _: T) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }
}

pub struct PipedProcessor<'a, 'b, I, T, O, P1, P2>
where
    P1: ProcessorTrait<I, T>,
    P2: ProcessorTrait<T, O>,
{
    _input: PhantomData<I>,
    _tmp: PhantomData<T>,
    _output: PhantomData<O>,
    processor1: &'a P1,
    processor2: &'b P2,
}

impl<'a, 'b, I, T, O, P1, P2> ProcessorTrait<I, O> for PipedProcessor<'a, 'b, I, T, O, P1, P2>
where
    P1: ProcessorTrait<I, T>,
    P2: ProcessorTrait<T, O>,
{
    async fn process(&self, input: I) -> Result<O, Box<dyn std::error::Error>> {
        self.processor2
            .process(self.processor1.process(input).await?)
            .await
    }
}
