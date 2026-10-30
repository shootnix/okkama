# Вызовы: рекурсивный fib (~7 млн вызовов).
class Fib
  def fib(n)
    n < 2 ? n : fib(n - 1) + fib(n - 2)
  end
end

puts Fib.new.fib(32)
