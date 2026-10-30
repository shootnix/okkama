use strict; use warnings;
# Приложение: модель, каталог, правила цены, ошибки домена, котировки и отчёт;
# неизменяемые значения, как в main.ok. В Perl нет async: котировка — вызов.

package Money;
sub new { bless { cents => $_[1] }, $_[0] }
sub cents { $_[0]{cents} }
sub plus { Money->new($_[0]{cents} + $_[1]{cents}) }
sub minus { Money->new($_[0]{cents} - $_[1]{cents}) }
sub times { Money->new($_[0]{cents} * $_[1]) }
sub percent { my $scaled = $_[0]{cents} * $_[1]; Money->new(($scaled - $scaled % 100) / 100) }
sub string {
    my $cents = $_[0]{cents};
    my $fraction = $cents % 100;
    my $whole = ($cents - $fraction) / 100;
    $whole . "." . ($fraction < 10 ? "0" : "") . $fraction;
}

package Product;
sub new { my ($class, %f) = @_; bless {%f}, $class }
sub price { Money->new($_[0]{price}) }

package Customer;
sub new { my ($class, %f) = @_; bless {%f}, $class }

package Line;
sub new { my ($class, %f) = @_; bless {%f}, $class }
sub subtotal { $_[0]{product}->price->times($_[0]{quantity}) }

package Order;
sub new { my ($class, %f) = @_; bless { lines => [], %f }, $class }
sub items { my $count = 0; $count += $_->{quantity} for @{ $_[0]{lines} }; $count }
sub subtotal { my $sum = Money->new(0); $sum = $sum->plus($_->subtotal) for @{ $_[0]{lines} }; $sum }
sub added { my ($self, $line) = @_; Order->new(id => $self->{id}, customer => $self->{customer}, lines => [@{ $self->{lines} }, $line]) }

package OutOfStock;
sub new { my ($class, %f) = @_; bless {%f}, $class }
sub text { "product " . $_[0]{product} . " is out of stock" }

package Catalog;
sub new { my ($class, %f) = @_; bless {%f}, $class }
sub product {
    my ($self, $id) = @_;
    Product->new(id => $id, name => "product-$id", category => $id * 7 % 10, price => $id * 389 % 20000 + 199, stock => $id * 13 % 40 + 1);
}
sub customer { my ($self, $id) = @_; Customer->new(id => $id, name => "customer-$id", tier => $id % 4) }
sub reserved {
    my ($self, $product, $quantity) = @_;
    die OutOfStock->new(product => $product->{id}, available => $product->{stock}) if $quantity > $product->{stock};
    $quantity;
}

package BulkDiscount;
sub new { bless {}, shift }
sub name { "bulk" }
sub adjusted { my ($self, $order, $total) = @_; $total->{cents} > 200000 ? $total->minus($total->percent(5)) : $total }

package LoyaltyDiscount;
sub new { bless {}, shift }
sub name { "loyalty" }
sub adjusted { my ($self, $order, $total) = @_; $total->minus($total->percent($order->{customer}{tier} * 2)) }

package CategoryPromotion;
sub new { my ($class, %f) = @_; bless {%f}, $class }
sub name { "promotion" }
sub adjusted {
    my ($self, $order, $total) = @_;
    my $discount = Money->new(0);
    for my $line (grep { $_->{product}{category} == $self->{category} } @{ $order->{lines} }) {
        $discount = $discount->plus($line->subtotal->percent(10));
    }
    $total->minus($discount);
}

package Tax;
sub new { bless {}, shift }
sub name { "tax" }
sub adjusted { my ($self, $order, $total) = @_; $total->plus($total->percent(20)) }

package PriceEngine;
sub new { my ($class, %f) = @_; bless {%f}, $class }
sub total {
    my ($self, $order) = @_;
    my $sum = $order->subtotal;
    $sum = $_->adjusted($order, $sum) for @{ $self->{rules} };
    $sum;
}

package OrderSource;
sub new { my ($class, %f) = @_; bless {%f}, $class }
sub order {
    my ($self, $id) = @_;
    my $catalog = $self->{catalog};
    my $order = Order->new(id => $id, customer => $catalog->customer($id * 31 % 5000));
    for my $index (0 .. $id % 8) {
        my $product = $catalog->product(($id * 97 + $index * 1031) % $catalog->{size});
        my $wanted = ($id + $index * 3) % 12 + 1;
        my $quantity = eval { $catalog->reserved($product, $wanted) };
        if (my $error = $@) {
            die $error unless ref $error && $error->isa('OutOfStock');
            $quantity = $error->{available};
        }
        $order = $order->added(Line->new(product => $product, quantity => $quantity));
    }
    $order;
}

package Report;
my @NAMES = qw(tools garden books music toys food sports office kitchen auto);
sub new { my ($class, %f) = @_; bless { revenue => [(0) x 10], orders => 0, items => 0, cents => 0, %f }, $class }
sub recorded {
    my ($self, $order, $total) = @_;
    my @lines = @{ $order->{lines} };
    my @revenue = map {
        my $category = $_;
        my $earned = 0;
        $earned += $_->subtotal->{cents} for grep { $_->{product}{category} == $category } @lines;
        $self->{revenue}[$category] + $earned;
    } 0 .. 9;
    Report->new(revenue => \@revenue, orders => $self->{orders} + 1, items => $self->{items} + $order->items, cents => $self->{cents} + $total->{cents});
}
sub text {
    my $self = shift;
    my @rows = map { $NAMES[$_] . ": " . Money->new($self->{revenue}[$_])->string } 0 .. 9;
    my $cents = $self->{cents};
    my $average = Money->new(($cents - $cents % $self->{orders}) / $self->{orders});
    join "\n", @rows, "orders: $self->{orders}", "items: $self->{items}", "revenue: " . Money->new($cents)->string, "average: " . $average->string;
}

package Quotes;
sub new { my ($class, %f) = @_; bless {%f}, $class }
sub quote { $_[0]{engine}->total($_[1]) }

package main;
my @rules = (BulkDiscount->new, CategoryPromotion->new(category => 3), LoyaltyDiscount->new, Tax->new);
my $quotes = Quotes->new(engine => PriceEngine->new(rules => \@rules));
my $source = OrderSource->new(catalog => Catalog->new(size => 2000));
my $report = Report->new;
for my $id (0 .. 29999) {
    my $order = $source->order($id);
    my $total = $quotes->quote($order);
    $report = $report->recorded($order, $total);
}
my $text = $report->text;
print "$text\n";
print "length: ", length($text), "\n";
