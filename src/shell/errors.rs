use expr;

error_chain!{
    links {
        ExpressionError(expr::Error, expr::ErrorKind);
    }

    errors {
        ExecutableNotInPath(exe : String) {
            description("the executable was not found $PATH"),
            display("could not find executable \"{}\" in your $PATH variable", exe)
        }
    }
}
