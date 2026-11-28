//! Plaintext expressions and viewport sampling for the graphing calculator.

#[derive(Clone, Debug, PartialEq)]
pub struct Expression(Node);

#[derive(Clone, Debug, PartialEq)]
enum Node {
    Number(f64),
    X,
    Negate(Box<Self>),
    Function(Function, Box<Self>),
    Binary(char, Box<Self>, Box<Self>),
}

impl Expression {
    pub fn parse(source: &str) -> Result<Self, &'static str> {
        if source.len() > 256 {
            return Err("Expression is too long");
        }
        let source = source.trim_start();
        let source = source
            .strip_prefix('y')
            .and_then(|rest| rest.trim_start().strip_prefix('='))
            .unwrap_or(source);
        let mut parser = Parser(source);
        let expression = parser.expression(0)?;
        if !parser.0.trim_start().is_empty() {
            return Err("Unexpected input");
        }
        Ok(Self(expression))
    }

    pub fn evaluate(&self, x: f64) -> f64 {
        self.0.evaluate(x)
    }

    /// Samples at half-column intervals to reject non-finite values and
    /// steep changes in slope near poles. Coordinates are in graph units.
    pub fn segments(
        &self,
        left: f64,
        right: f64,
        columns: usize,
        units_per_pixel: f64,
    ) -> impl Iterator<Item = [(f64, f64); 2]> + '_ {
        let step = (right - left) / columns.max(1) as f64;
        let mut previous = self.evaluate(left);
        (0..columns).filter_map(move |column| {
            let x = left + step * column as f64;
            let next = self.evaluate(x + step);
            let middle = self.evaluate(x + step * 0.5);
            let start = std::mem::replace(&mut previous, next);
            let error = (middle - (start * 0.5 + next * 0.5)).abs();
            (start.is_finite() && next.is_finite() && error <= units_per_pixel * 12.0)
                .then_some([(x, start), (x + step, next)])
        })
    }
}

impl Node {
    fn evaluate(&self, x: f64) -> f64 {
        match self {
            Self::Number(n) => *n,
            Self::X => x,
            Self::Negate(value) => -value.evaluate(x),
            Self::Function(function, value) => function.evaluate(value.evaluate(x)),
            Self::Binary(operator, left, right) => {
                let left = left.evaluate(x);
                let right = right.evaluate(x);
                match operator {
                    '+' => left + right,
                    '-' => left - right,
                    '*' => left * right,
                    '/' => left / right,
                    '^' => left.powf(right),
                    _ => unreachable!(),
                }
            }
        }
    }

}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Function {
    Sin, Cos, Tan, Sqrt, Abs, Ln,
}

impl Function {
    fn evaluate(self, value: f64) -> f64 {
        match self {
            Self::Sin => value.sin(),
            Self::Cos => value.cos(),
            Self::Tan => value.tan(),
            Self::Sqrt => value.sqrt(),
            Self::Abs => value.abs(),
            Self::Ln => value.ln(),
        }
    }
}

struct Parser<'a>(&'a str);

impl Parser<'_> {
    fn take(&mut self, token: char) -> bool {
        self.0 = self.0.trim_start();
        if let Some(rest) = self.0.strip_prefix(token) {
            self.0 = rest;
            return true;
        }
        false
    }

    fn expression(&mut self, minimum: u8) -> Result<Node, &'static str> {
        let mut left = self.atom()?;
        loop {
            self.0 = self.0.trim_start();
            let Some(operator) = self.0.chars().next() else {
                break;
            };
            let (precedence, right) = match operator {
                '+' | '-' => (1, 2),
                '*' | '/' => (3, 4),
                '^' => (6, 6),
                _ => break,
            };
            if precedence < minimum {
                break;
            }
            self.0 = &self.0[1..];
            left = Node::Binary(operator, Box::new(left), Box::new(self.expression(right)?));
        }
        Ok(left)
    }

    fn atom(&mut self) -> Result<Node, &'static str> {
        if self.take('-') {
            return Ok(Node::Negate(Box::new(self.expression(5)?)));
        }
        if self.take('+') {
            return self.expression(5);
        }
        if self.take('(') {
            return self.group();
        }
        let name = self.0.bytes().take_while(u8::is_ascii_alphabetic).count();
        if name > 0 {
            let (identifier, rest) = self.0.split_at(name);
            self.0 = rest;
            let function = match identifier {
                "x" => return Ok(Node::X),
                "pi" => return Ok(Node::Number(std::f64::consts::PI)),
                "e" => return Ok(Node::Number(std::f64::consts::E)),
                "sin" => Function::Sin,
                "cos" => Function::Cos,
                "tan" => Function::Tan,
                "sqrt" => Function::Sqrt,
                "abs" => Function::Abs,
                "ln" => Function::Ln,
                _ => return Err("Unknown name"),
            };
            if !self.take('(') {
                return Err("Expected an opening parenthesis");
            }
            return Ok(Node::Function(function, Box::new(self.group()?)));
        }
        let number = self.0.bytes().take_while(|byte| byte.is_ascii_digit() || *byte == b'.').count();
        let (number, rest) = self.0.split_at(number);
        self.0 = rest;
        number.parse().map(Node::Number).map_err(|_| "Expected a number or expression")
    }

    fn group(&mut self) -> Result<Node, &'static str> {
        let expression = self.expression(0)?;
        if !self.take(')') {
            return Err("Expected a closing parenthesis");
        }
        Ok(expression)
    }
}
